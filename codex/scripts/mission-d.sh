#!/usr/bin/env bash
set -u

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$ROOT"

mkdir -p codex/logs codex/reports
STAMP="$(date +%Y%m%d-%H%M%S)"
UI_LOG="codex/logs/mission-d-ui-${STAMP}.log"
BACKEND_LOG="codex/logs/mission-d-backend-${STAMP}.log"
REPORT_TS="codex/reports/mission-d-${STAMP}.md"
REPORT_LATEST="codex/reports/mission-d-latest.md"

UI_CMD="npm test -- src/tests/OperationsPanel.test.js"
BACKEND_CMD_1="cargo test --manifest-path src-tauri/Cargo.toml validate_output_name_"
BACKEND_CMD_2="cargo test --manifest-path src-tauri/Cargo.toml validate_output_dir_"
BACKEND_CMD_3="cargo test --manifest-path src-tauri/Cargo.toml op_result_arbitrary_error_produces_error_variant"

UI_STATUS="PASS"
BACKEND_STATUS="PASS"

set +e
$UI_CMD >"$UI_LOG" 2>&1
UI_CODE=$?
$BACKEND_CMD_1 >"$BACKEND_LOG" 2>&1
BACKEND_CODE_1=$?
$BACKEND_CMD_2 >>"$BACKEND_LOG" 2>&1
BACKEND_CODE_2=$?
$BACKEND_CMD_3 >>"$BACKEND_LOG" 2>&1
BACKEND_CODE_3=$?
set -e

if [ "$UI_CODE" -ne 0 ]; then
  UI_STATUS="FAIL"
fi
if [ "$BACKEND_CODE_1" -ne 0 ] || [ "$BACKEND_CODE_2" -ne 0 ] || [ "$BACKEND_CODE_3" -ne 0 ]; then
  BACKEND_STATUS="FAIL"
fi

OVERALL="PASS"
if [ "$UI_STATUS" != "PASS" ] || [ "$BACKEND_STATUS" != "PASS" ]; then
  OVERALL="FAIL"
fi

cat > "$REPORT_TS" <<REPORT
# Mission D Report

- Mission: Error-Surface Quality
- Date: $(date -u +"%Y-%m-%d %H:%M:%S UTC")
- Overall: ${OVERALL}

## Checks
1. UI operations error-path coverage
- Command: ${UI_CMD}
- Status: ${UI_STATUS}
- Log: ${UI_LOG}

2. Backend validation/error classification checks
- Command 1: ${BACKEND_CMD_1}
- Command 2: ${BACKEND_CMD_2}
- Command 3: ${BACKEND_CMD_3}
- Status: ${BACKEND_STATUS}
- Log: ${BACKEND_LOG}

## Notes
- This mission validates invalid output path handling and error-result classification behavior.
REPORT

cp "$REPORT_TS" "$REPORT_LATEST"

echo "Mission D complete: ${OVERALL}"
echo "Report: ${REPORT_TS}"
echo "Latest: ${REPORT_LATEST}"
echo "UI log: ${UI_LOG}"
echo "Backend log: ${BACKEND_LOG}"

if [ "$OVERALL" != "PASS" ]; then
  exit 1
fi
