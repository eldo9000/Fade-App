# Codex Mission Scripts

These scripts run Codex-owned pilot missions and emit logs/reports under `codex/`.

Prerequisites:
- Run from repo root.
- Keep dev app open in a separate terminal (`npm run tauri dev`) for UI mission context.

Commands:
- `./codex/scripts/mission-a.sh`  # MP4 basic conversion obedience
- `./codex/scripts/mission-b.sh`  # MP4 codec/resolution/fps stress
- `./codex/scripts/mission-c.sh`  # MP4 resolution/fps persistence
- `./codex/scripts/mission-d.sh`  # Error-surface quality checks
- `./codex/scripts/run-all-missions.sh`  # Run A+B+C+D and generate suite summary
- `./codex/scripts/run-matrix.sh`  # Run enabled matrix rows from codex/matrix/mission-matrix.csv

Matrix filters:
- `./codex/scripts/run-matrix.sh --lane prompt`
- `./codex/scripts/run-matrix.sh --severity P1`
- `./codex/scripts/run-matrix.sh --case M-B-001`
- `./codex/scripts/run-matrix.sh --lane prompt --severity P1`
- `./codex/scripts/run-matrix.sh --list --lane prompt`  # Preview matching rows without running

Outputs:
- `codex/logs/` command logs per run
- `codex/reports/*-latest.md` latest mission summaries
- `codex/reports/missions-suite-latest.md` latest combined suite summary
- `codex/reports/matrix-run-latest.md` latest matrix summary
- `codex/artifacts/matrix/matrix-run-latest.csv` latest machine-readable matrix results
