#!/usr/bin/env bash
set -u

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$ROOT"

mkdir -p codex/reports codex/logs
STAMP="$(date +%Y%m%d-%H%M%S)"
SUITE_REPORT="codex/reports/missions-suite-${STAMP}.md"
SUITE_LATEST="codex/reports/missions-suite-latest.md"

MISSIONS=(
  "A:./codex/scripts/mission-a.sh:codex/reports/mission-a-latest.md"
  "B:./codex/scripts/mission-b.sh:codex/reports/mission-b-latest.md"
  "C:./codex/scripts/mission-c.sh:codex/reports/mission-c-latest.md"
  "D:./codex/scripts/mission-d.sh:codex/reports/mission-d-latest.md"
)

PASS_COUNT=0
TOTAL_COUNT=0

{
  echo "# Mission Suite Report"
  echo
  echo "- Date: $(date -u +"%Y-%m-%d %H:%M:%S UTC")"
  echo "- Scope: Mission A + B + C + D"
  echo
  echo "## Results"
} > "$SUITE_REPORT"

for entry in "${MISSIONS[@]}"; do
  IFS=':' read -r mission_name mission_cmd mission_report <<< "$entry"
  TOTAL_COUNT=$((TOTAL_COUNT + 1))

  echo "Running Mission ${mission_name}..."
  set +e
  $mission_cmd
  code=$?
  set -e

  status="PASS"
  if [ "$code" -ne 0 ]; then
    status="FAIL"
  else
    PASS_COUNT=$((PASS_COUNT + 1))
  fi

  {
    echo "- Mission ${mission_name}: ${status}"
    echo "  - Command: ${mission_cmd}"
    echo "  - Latest report: ${mission_report}"
  } >> "$SUITE_REPORT"
done

OVERALL="PASS"
if [ "$PASS_COUNT" -ne "$TOTAL_COUNT" ]; then
  OVERALL="FAIL"
fi

{
  echo
  echo "## Summary"
  echo "- Overall: ${OVERALL}"
  echo "- Passed: ${PASS_COUNT}/${TOTAL_COUNT}"
} >> "$SUITE_REPORT"

cp "$SUITE_REPORT" "$SUITE_LATEST"

echo
echo "Suite complete: ${OVERALL} (${PASS_COUNT}/${TOTAL_COUNT})"
echo "Report: ${SUITE_REPORT}"
echo "Latest: ${SUITE_LATEST}"

if [ "$OVERALL" != "PASS" ]; then
  exit 1
fi
