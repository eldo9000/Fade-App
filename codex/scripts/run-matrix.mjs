#!/usr/bin/env node
import { readFileSync, writeFileSync, mkdirSync, existsSync } from 'node:fs';
import { resolve } from 'node:path';
import { spawnSync } from 'node:child_process';

function utcNow() {
  return new Date().toISOString().replace('T', ' ').replace('Z', ' UTC');
}

function stampNow() {
  const d = new Date();
  const pad = (n) => String(n).padStart(2, '0');
  return [
    d.getFullYear(),
    pad(d.getMonth() + 1),
    pad(d.getDate()),
    '-',
    pad(d.getHours()),
    pad(d.getMinutes()),
    pad(d.getSeconds()),
  ].join('');
}

function parseCsv(csvText) {
  const lines = csvText
    .split(/\r?\n/)
    .map((l) => l.trim())
    .filter((l) => l.length > 0 && !l.startsWith('#'));

  if (lines.length < 2) return [];

  const header = lines[0].split(',').map((h) => h.trim());
  return lines.slice(1).map((line) => {
    const cells = line.split(',');
    const row = {};
    for (let i = 0; i < header.length; i += 1) {
      row[header[i]] = (cells[i] ?? '').trim();
    }
    return row;
  });
}

function splitArgValues(value) {
  return String(value)
    .split(',')
    .map((v) => v.trim())
    .filter((v) => v.length > 0);
}

function parseArgs(argv) {
  const options = {
    lanes: new Set(),
    severities: new Set(),
    cases: new Set(),
    listOnly: false,
  };

  for (let i = 0; i < argv.length; i += 1) {
    const arg = argv[i];
    if (arg === '--list') {
      options.listOnly = true;
      continue;
    }

    if (arg === '--lane') {
      const value = argv[i + 1];
      if (!value) {
        throw new Error('--lane requires a value');
      }
      splitArgValues(value).forEach((v) => options.lanes.add(v));
      i += 1;
      continue;
    }

    if (arg === '--severity') {
      const value = argv[i + 1];
      if (!value) {
        throw new Error('--severity requires a value');
      }
      splitArgValues(value).forEach((v) => options.severities.add(v));
      i += 1;
      continue;
    }

    if (arg === '--case') {
      const value = argv[i + 1];
      if (!value) {
        throw new Error('--case requires a value');
      }
      splitArgValues(value).forEach((v) => options.cases.add(v));
      i += 1;
      continue;
    }

    throw new Error(`Unknown argument: ${arg}`);
  }

  return options;
}

function applyFilters(rows, options) {
  return rows.filter((r) => {
    if (options.lanes.size > 0 && !options.lanes.has(r.lane)) return false;
    if (options.severities.size > 0 && !options.severities.has(r.severity)) return false;
    if (options.cases.size > 0 && !options.cases.has(r.case_id)) return false;
    return true;
  });
}

function runCase(root, row, stamp) {
  const caseLog = resolve(root, `codex/logs/matrix-${row.case_id}-${stamp}.log`);
  const cmd = row.command;
  const res = spawnSync(cmd, {
    cwd: root,
    shell: true,
    encoding: 'utf8',
  });

  writeFileSync(
    caseLog,
    [
      `# Matrix Case Log`,
      `case_id=${row.case_id}`,
      `mission=${row.mission}`,
      `command=${cmd}`,
      `exit_code=${res.status ?? 1}`,
      '',
      '--- stdout ---',
      res.stdout ?? '',
      '',
      '--- stderr ---',
      res.stderr ?? '',
      '',
    ].join('\n'),
    'utf8'
  );

  const expectedReportAbs = resolve(root, row.expected_report);
  const reportExists = existsSync(expectedReportAbs);
  const pass = (res.status === 0) && reportExists;

  return {
    case_id: row.case_id,
    mission: row.mission,
    lane: row.lane,
    severity: row.severity,
    status: pass ? 'PASS' : 'FAIL',
    exit_code: res.status ?? 1,
    command: cmd,
    expected_report: row.expected_report,
    report_exists: reportExists ? 'yes' : 'no',
    log_path: caseLog,
  };
}

function main() {
  let options;
  try {
    options = parseArgs(process.argv.slice(2));
  } catch (err) {
    console.error(`Argument error: ${err.message}`);
    process.exit(1);
  }

  const root = process.cwd();
  const matrixPath = resolve(root, 'codex/matrix/mission-matrix.csv');
  const matrixText = readFileSync(matrixPath, 'utf8');
  const enabledRows = parseCsv(matrixText).filter((r) => r.enabled.toLowerCase() === 'true');
  const rows = applyFilters(enabledRows, options);

  if (rows.length === 0) {
    console.error('No matrix rows matched the current filters.');
    process.exit(1);
  }

  if (options.listOnly) {
    console.log(`Matched ${rows.length} row(s):`);
    for (const row of rows) {
      console.log(`- ${row.case_id} [lane=${row.lane}, severity=${row.severity}] ${row.mission}`);
    }
    return;
  }

  mkdirSync(resolve(root, 'codex/reports'), { recursive: true });
  mkdirSync(resolve(root, 'codex/logs'), { recursive: true });
  mkdirSync(resolve(root, 'codex/artifacts/matrix'), { recursive: true });

  const stamp = stampNow();
  const results = rows.map((row) => runCase(root, row, stamp));

  const passCount = results.filter((r) => r.status === 'PASS').length;
  const totalCount = results.length;
  const overall = passCount === totalCount ? 'PASS' : 'FAIL';

  const reportPath = resolve(root, `codex/reports/matrix-run-${stamp}.md`);
  const latestReportPath = resolve(root, 'codex/reports/matrix-run-latest.md');
  const artifactCsvPath = resolve(root, `codex/artifacts/matrix/matrix-run-${stamp}.csv`);
  const artifactCsvLatestPath = resolve(root, 'codex/artifacts/matrix/matrix-run-latest.csv');

  const reportLines = [
    '# Matrix Run Report',
    '',
    `- Date: ${utcNow()}`,
    `- Filters: lane=${options.lanes.size ? [...options.lanes].join('|') : 'all'}, severity=${options.severities.size ? [...options.severities].join('|') : 'all'}, case=${options.cases.size ? [...options.cases].join('|') : 'all'}`,
    `- Overall: ${overall}`,
    `- Passed: ${passCount}/${totalCount}`,
    '',
    '## Cases',
  ];

  for (const r of results) {
    reportLines.push(`- ${r.case_id} (${r.mission}): ${r.status}`);
    reportLines.push(`  - Command: ${r.command}`);
    reportLines.push(`  - Expected report: ${r.expected_report}`);
    reportLines.push(`  - Report exists: ${r.report_exists}`);
    reportLines.push(`  - Log: ${r.log_path}`);
  }

  writeFileSync(reportPath, reportLines.join('\n') + '\n', 'utf8');
  writeFileSync(latestReportPath, reportLines.join('\n') + '\n', 'utf8');

  const csvHeader = 'case_id,mission,lane,severity,status,exit_code,report_exists,expected_report,log_path';
  const csvLines = results.map((r) => (
    [r.case_id, r.mission, r.lane, r.severity, r.status, r.exit_code, r.report_exists, r.expected_report, r.log_path]
      .map((v) => `"${String(v).replaceAll('"', '""')}"`)
      .join(',')
  ));
  const csvOut = [csvHeader, ...csvLines].join('\n') + '\n';
  writeFileSync(artifactCsvPath, csvOut, 'utf8');
  writeFileSync(artifactCsvLatestPath, csvOut, 'utf8');

  console.log(`Matrix run complete: ${overall} (${passCount}/${totalCount})`);
  console.log(`Report: ${reportPath}`);
  console.log(`Latest: ${latestReportPath}`);
  console.log(`Artifact: ${artifactCsvPath}`);
  console.log(`Artifact latest: ${artifactCsvLatestPath}`);

  if (overall !== 'PASS') {
    process.exit(1);
  }
}

main();
