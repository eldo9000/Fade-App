// ── Diagnostics ring buffer ──────────────────────────────────────────────────
// In-memory log of errors and other notable events. Bounded — oldest entries
// drop off once the cap is hit. Used by the Diagnostics section in Settings
// so the user can see recent failures and copy a report when filing a bug.
//
// This file is intentionally local-only. No network calls, no disk writes.
// A future crash-reporter task can add an opt-in uploader that reads from
// here; until then, the user copies/pastes manually.

const MAX_ENTRIES = 100;

/** @type {{ t: number, source: string, message: string, detail?: string }[]} */
const entries = $state([]);

/**
 * Append an error entry. `source` is a short label ('updater', 'ffprobe',
 * 'window.onerror' …). `detail` is optional multi-line context (stack,
 * payload). Anything thrown gets coerced to a string; we never re-throw.
 */
export function pushError(source, message, detail) {
  try {
    const entry = {
      t: Date.now(),
      source: String(source ?? 'unknown'),
      message: String(message ?? ''),
    };
    if (detail !== undefined && detail !== null) {
      entry.detail = typeof detail === 'string'
        ? detail
        : (detail instanceof Error ? (detail.stack ?? detail.message ?? String(detail)) : String(detail));
    }
    entries.push(entry);
    while (entries.length > MAX_ENTRIES) entries.shift();
    // Keep the console trail too — helps when the devtools are open during a repro.
    console.warn(`[${entry.source}] ${entry.message}`, detail ?? '');
  } catch {
    // Never let diagnostics themselves blow up the caller.
  }
}

export function clearDiagnostics() {
  entries.length = 0;
}

export function getEntries() { return entries; }

/**
 * Formatted plain-text snapshot — suitable for clipboard paste into a
 * GitHub issue. `header` lets the caller prepend app/platform info.
 */
export function snapshotText(header = '') {
  const lines = [];
  if (header) lines.push(header, '');
  if (entries.length === 0) {
    lines.push('(no diagnostics recorded this session)');
  } else {
    for (const e of entries) {
      const ts = new Date(e.t).toISOString();
      lines.push(`[${ts}] [${e.source}] ${e.message}`);
      if (e.detail) lines.push(e.detail.split('\n').map(l => '  ' + l).join('\n'));
    }
  }
  return lines.join('\n');
}
