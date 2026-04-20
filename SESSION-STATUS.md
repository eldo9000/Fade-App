# Fade — Session Status

Last updated: 2026-04-20

---

## Current Focus

Pre-1.0 polish phase. v0.6.x series shipping incremental UX fixes and dev-feature gating. Recent work: chromakey/colorkey, codec presets, sidebar search, settings panel fixes, macOS Gatekeeper signing.

## Next action

**Fix the v0.6.1 release.** The Release workflow failed on tag v0.6.1 because `src-tauri/tauri.conf.json` version (`0.6.0`) doesn't match the git tag (`0.6.1`). Either bump `tauri.conf.json` to `0.6.1` and re-tag, or cut a fresh `0.6.2` that bundles the Gatekeeper signing fix.

Success signal: Release workflow run completes green and binaries appear on the GitHub Releases page.

## Known Risks

- **Release workflow brittle.** The version skew check is strict — every release must bump `src-tauri/tauri.conf.json` in the same commit as the tag, or the workflow dies on "tag version does not match". Document the release ritual or add a bump-and-tag script.
- **Release pipeline uses Node.js 20 actions.** Deprecation warning from GitHub; `actions/checkout@v4` flagged. Node 24 becomes default on 2026-06-02. Not urgent, but on the clock.
- **CI on main is green** (run 24648547365). Release workflow is the only broken pipeline.

## Mode

Pre-ship polish. Feature surface is functionally complete; remaining work is stability, release hygiene, and final UX passes before 1.0.
