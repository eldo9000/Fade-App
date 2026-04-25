# TASK-3: Harden GHA release.yml against shell injection via inputs.tag

## Goal
`${{ inputs.tag }}` is no longer interpolated directly into `run:` shell steps in `.github/workflows/release.yml`; the tag value is passed through an intermediate `env:` variable instead.

## Context
`.github/workflows/release.yml` has two "Resolve tag" steps — one in the `build` job and one in the `publish` job — that use `${{ inputs.tag }}` directly inside `run:` blocks:

```yaml
run: |
  if [ "${{ github.event_name }}" = "workflow_dispatch" ]; then
    TAG="${{ inputs.tag }}"
  ...
```

Direct interpolation of `${{ ... }}` expressions into `run:` blocks is a known GHA shell injection vector: if the expression evaluates to a string containing shell metacharacters (semicolons, backticks, `$(...)`, etc.), the shell will execute them. For `inputs.tag` this requires someone with `workflow_dispatch` permission to manually trigger the workflow with a crafted tag value — not easily exploitable on a private or protected repo, but a hygiene violation that GHA security guidelines flag.

**The fix:** Move the `${{ inputs.tag }}` interpolation into an `env:` key on the step. Environment variables set via `env:` are passed to the shell as named variables and are never interpreted as shell code, regardless of their content.

Before:
```yaml
- name: Resolve tag
  run: |
    if [ "${{ github.event_name }}" = "workflow_dispatch" ]; then
      TAG="${{ inputs.tag }}"
```

After:
```yaml
- name: Resolve tag
  env:
    INPUT_TAG: ${{ inputs.tag }}
  run: |
    if [ "${{ github.event_name }}" = "workflow_dispatch" ]; then
      TAG="$INPUT_TAG"
```

The `${{ github.event_name }}` interpolation is safe to keep inline because `github.event_name` is a GitHub-controlled value (always `"push"` or `"workflow_dispatch"`) and cannot be influenced by user input.

**Both occurrences must be fixed:** There are two "Resolve tag" steps — one in the `build` job (around line 112) and one in the `publish` job (around line 368). Both need the same treatment.

## In scope
- `.github/workflows/release.yml` — two "Resolve tag" steps only

## Out of scope
- Any other jobs or steps in `release.yml`
- Any other workflow files
- Any Rust or frontend source files

## Steps
1. Read `.github/workflows/release.yml` in full to understand the structure and locate both "Resolve tag" steps.
2. For each "Resolve tag" step:
   a. Add an `env:` key to the step with `INPUT_TAG: ${{ inputs.tag }}`.
   b. Replace `TAG="${{ inputs.tag }}"` in the `run:` block with `TAG="$INPUT_TAG"`.
3. Verify no other `${{ inputs.tag }}` interpolations exist elsewhere in `run:` blocks in the file (a grep is sufficient).
4. Confirm the YAML is still valid (correct indentation, no broken structure).
5. No build check is needed — this is YAML only and CI will validate it on push.

## Success signal
- `.github/workflows/release.yml` contains zero occurrences of `${{ inputs.tag }}` inside `run:` blocks.
- Both "Resolve tag" steps have an `env:` key with `INPUT_TAG: ${{ inputs.tag }}` and use `"$INPUT_TAG"` in the shell body.
- All other content in the file is unchanged.
