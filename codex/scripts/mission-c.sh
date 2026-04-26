#!/usr/bin/env bash
set -u

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$ROOT"

mkdir -p codex/logs codex/reports
STAMP="$(date +%Y%m%d-%H%M%S)"
UI_LOG="codex/logs/mission-c-ui-${STAMP}.log"
ARGS_LOG="codex/logs/mission-c-arg-tests-${STAMP}.log"
REPORT_TS="codex/reports/mission-c-${STAMP}.md"
REPORT_LATEST="codex/reports/mission-c-latest.md"

UI_CMD="npm run test:e2e -- e2e/specs/video-options.spec.ts"
ARGS_CMD_1="cargo test --manifest-path src-tauri/Cargo.toml video_args_frame_rate_"
ARGS_CMD_2="cargo test --manifest-path src-tauri/Cargo.toml video_args_resolution_"

UI_STATUS="PASS"
ARGS_STATUS="PASS"

set +e
$UI_CMD >"$UI_LOG" 2>&1
UI_CODE=$?
$ARGS_CMD_1 >"$ARGS_LOG" 2>&1
ARGS_CODE_1=$?
$ARGS_CMD_2 >>"$ARGS_LOG" 2>&1
ARGS_CODE_2=$?
set -e

if [ "$UI_CODE" -ne 0 ]; then
  UI_STATUS="FAIL"
fi
if [ "$ARGS_CODE_1" -ne 0 ] || [ "$ARGS_CODE_2" -ne 0 ]; then
  ARGS_STATUS="FAIL"
fi

OVERALL="PASS"
if [ "$UI_STATUS" != "PASS" ] || [ "$ARGS_STATUS" != "PASS" ]; then
  OVERALL="FAIL"
fi

cat > "$REPORT_TS" <<REPORT
# Mission C Report

- Mission: MP4 Resolution/FPS Persistence
- Date: $(date -u +"%Y-%m-%d %H:%M:%S UTC")
- Overall: ${OVERALL}

## Checks
1. UI resolution/fps interaction sweep
- Command: ${UI_CMD}
- Status: ${UI_STATUS}
- Log: ${UI_LOG}

2. Backend resolution/fps argument regression tests
- Command 1: ${ARGS_CMD_1}
- Command 2: ${ARGS_CMD_2}
- Status: ${ARGS_STATUS}
- Log: ${ARGS_LOG}

## Notes
- This mission targets persistence/regression in resolution and frame-rate selections around MP4/video workflows.
REPORT

cp "$REPORT_TS" "$REPORT_LATEST"

echo "Mission C complete: ${OVERALL}"
echo "Report: ${REPORT_TS}"
echo "Latest: ${REPORT_LATEST}"
echo "UI log: ${UI_LOG}"
echo "Arg-tests log: ${ARGS_LOG}"

if [ "$OVERALL" != "PASS" ]; then
  exit 1
fi
