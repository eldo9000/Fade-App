#!/usr/bin/env bash
set -u

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$ROOT"

mkdir -p codex/logs codex/reports
STAMP="$(date +%Y%m%d-%H%M%S)"
UI_LOG="codex/logs/mission-b-ui-${STAMP}.log"
ARGS_LOG="codex/logs/mission-b-arg-tests-${STAMP}.log"
REPORT_TS="codex/reports/mission-b-${STAMP}.md"
REPORT_LATEST="codex/reports/mission-b-latest.md"

UI_CMD="npm run test:e2e -- e2e/specs/video-options.spec.ts"
ARGS_CMD="cargo test --manifest-path src-tauri/Cargo.toml video_args_"

UI_STATUS="PASS"
ARGS_STATUS="PASS"

set +e
$UI_CMD >"$UI_LOG" 2>&1
UI_CODE=$?
$ARGS_CMD >"$ARGS_LOG" 2>&1
ARGS_CODE=$?
set -e

if [ "$UI_CODE" -ne 0 ]; then
  UI_STATUS="FAIL"
fi
if [ "$ARGS_CODE" -ne 0 ]; then
  ARGS_STATUS="FAIL"
fi

OVERALL="PASS"
if [ "$UI_STATUS" != "PASS" ] || [ "$ARGS_STATUS" != "PASS" ]; then
  OVERALL="FAIL"
fi

cat > "$REPORT_TS" <<REPORT
# Mission B Report

- Mission: MP4 Codec Stress (Codex-only pilot)
- Date: $(date -u +"%Y-%m-%d %H:%M:%S UTC")
- Overall: ${OVERALL}

## Checks
1. UI codec/resolution/fps stress sweep
- Command: ${UI_CMD}
- Status: ${UI_STATUS}
- Log: ${UI_LOG}

2. Backend video argument regression tests
- Command: ${ARGS_CMD}
- Status: ${ARGS_STATUS}
- Log: ${ARGS_LOG}

## Notes
- This mission targets state reset bugs and codec option regressions around MP4/video workflows.
REPORT

cp "$REPORT_TS" "$REPORT_LATEST"

echo "Mission B complete: ${OVERALL}"
echo "Report: ${REPORT_TS}"
echo "Latest: ${REPORT_LATEST}"
echo "UI log: ${UI_LOG}"
echo "Arg-tests log: ${ARGS_LOG}"

if [ "$OVERALL" != "PASS" ]; then
  exit 1
fi
