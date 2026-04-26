# Prompt Mission Template

## Mission ID
`mission-<area>-<date>-<seq>`

## User Prompt
<copy exact user request>

## Normalized Objective
- Target area:
- Primary behavior to test:
- Timebox:
- Stop condition:

## Constraints
- Allowed scope:
- Forbidden scope:
- Allowed edits (yes/no):
- Do not rename outputs or create new suffixes unless the mission explicitly asks for it.
- Assume the output folder is already cleared before the run unless the mission says otherwise.
- If a conversion action is required, name the exact button to click.
- If the expected button is not visible or does not respond, re-check the current UI state and stop with a report instead of guessing.

## Planned Checks
1. Confirm the correct file is selected.
2. Confirm the target format panel is open.
3. Click the exact convert control the mission specifies.
4. Verify the UI state changes after the click.
5. Verify the output file appears in the expected folder.
6. Record any mismatch between intended action and observed result.

## Evidence Targets
- Logs:
- Screenshots/artifacts:
- Output files:
- UI state notes:

## Completion Criteria
- [ ] Mission objective satisfied
- [ ] Reproducible findings captured
- [ ] Verification summary produced
