#!/usr/bin/env bash
set -u

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$ROOT"

mkdir -p codex/logs codex/reports
STAMP="$(date +%Y%m%d-%H%M%S)"
UI_LOG="codex/logs/mission-a-ui-${STAMP}.log"
CONV_LOG="codex/logs/mission-a-conversion-${STAMP}.log"
REPORT_TS="codex/reports/mission-a-${STAMP}.md"
REPORT_LATEST="codex/reports/mission-a-latest.md"

UI_CMD="npm run test:e2e -- e2e/specs/video-options.spec.ts"
CONV_CMD="cargo test --manifest-path src-tauri/Cargo.toml --test conversions video_mp4_to_webm -- --include-ignored"

UI_STATUS="PASS"
CONV_STATUS="PASS"

set +e
$UI_CMD >"$UI_LOG" 2>&1
UI_CODE=$?
$CONV_CMD >"$CONV_LOG" 2>&1
CONV_CODE=$?
set -e

if [ "$UI_CODE" -ne 0 ]; then
  UI_STATUS="FAIL"
fi
if [ "$CONV_CODE" -ne 0 ]; then
  CONV_STATUS="FAIL"
fi

OVERALL="PASS"
if [ "$UI_STATUS" != "PASS" ] || [ "$CONV_STATUS" != "PASS" ]; then
  OVERALL="FAIL"
fi

cat > "$REPORT_TS" <<REPORT
# Mission A Report

- Mission: MP4 Basic Conversion Obedience (Codex-only pilot)
- Date: $(date -u +"%Y-%m-%d %H:%M:%S UTC")
- Overall: ${OVERALL}

## Checks
1. UI video-options interaction sweep
- Command: ${UI_CMD}
- Status: ${UI_STATUS}
- Log: ${UI_LOG}

2. Backend conversion validation (MP4 -> WebM)
- Command: ${CONV_CMD}
- Status: ${CONV_STATUS}
- Log: ${CONV_LOG}

## Notes
- This Codex-only pilot validates UI interaction coverage plus conversion correctness without external computer-use APIs.
REPORT

cp "$REPORT_TS" "$REPORT_LATEST"

echo "Mission A complete: ${OVERALL}"
echo "Report: ${REPORT_TS}"
echo "Latest: ${REPORT_LATEST}"
echo "UI log: ${UI_LOG}"
echo "Conversion log: ${CONV_LOG}"

if [ "$OVERALL" != "PASS" ]; then
  exit 1
fi
