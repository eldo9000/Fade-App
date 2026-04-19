// ── Diagnostics ring buffer ──────────────────────────────────────────────────
// In-memory log of errors and other notable events, backed by a JSONL file
// under the Tauri app-log dir (`diag_append` / `diag_load` / `diag_clear`).
// The buffer mirrors what's on disk so the Diagnostics panel shows history
// across restarts.
//
// This file is intentionally local-only. A future opt-in crash uploader
// reads from the same buffer and POSTs to a collector; until then, the
// user copies/pastes manually.

import { invoke } from '@tauri-apps/api/core';

const MAX_ENTRIES = 100;

// ── Crash/diagnostics uploader config ────────────────────────────────────────
// Beta-forced-on: until BETA flips to false, the Settings toggle is locked
// ON with a red warning copy ("you are beta-testing this build; diagnostics
// will be submitted with updates"). After BETA → false, the toggle becomes
// a normal opt-in with the same default-on.
//
// CRASH_ENDPOINT is the collector URL. Leave empty to disable the uploader
// at the network layer — buttons still work, `uploadDiagnostics()` is a
// no-op until an endpoint is set. We intentionally don't ship a fallback
// URL so no traffic leaves the machine before you choose the infra.
export const BETA = true;
const CRASH_ENDPOINT = 'https://fade-crash-reporter.libre-apps.workers.dev/report';
const UPLOAD_TIMEOUT_MS = 5000;

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
    // Fire-and-forget disk append. Failures here are themselves logged to the
    // console only (avoid a write-failure feedback loop into pushError).
    invoke('diag_append', { entry }).catch((e) => {
      console.warn('[diagnostics] persist failed', e);
    });
  } catch {
    // Never let diagnostics themselves blow up the caller.
  }
}

export async function loadPersisted() {
  try {
    const rows = await invoke('diag_load');
    if (Array.isArray(rows) && rows.length > 0) {
      // Prepend historical entries so new in-session errors append after them,
      // preserving chronological order. Respect the cap.
      const merged = rows.concat(entries);
      entries.length = 0;
      const start = Math.max(0, merged.length - MAX_ENTRIES);
      for (let i = start; i < merged.length; i++) entries.push(merged[i]);
    }
  } catch (e) {
    console.warn('[diagnostics] load failed', e);
  }
}

export function clearDiagnostics() {
  entries.length = 0;
  invoke('diag_clear').catch((e) => console.warn('[diagnostics] clear failed', e));
}

/**
 * Upload the current buffer to the collector. Piggybacks on the user's
 * explicit "Update Now" / "Download update" click — no background traffic,
 * ever. Returns true if something was sent (or would've been), false if
 * the uploader is disabled or there's nothing to send.
 *
 * `meta` is callsite-provided context (app version, user agent, platform).
 * Failures are swallowed and logged — we don't want a dead collector to
 * block the update flow the user actually wanted.
 */
export async function uploadDiagnostics({ enabled, meta }) {
  if (!enabled) return false;
  if (!CRASH_ENDPOINT) return false;
  if (entries.length === 0) return false;
  const payload = {
    ...meta,
    beta: BETA,
    entries: entries.slice(),
  };
  const controller = new AbortController();
  const timer = setTimeout(() => controller.abort(), UPLOAD_TIMEOUT_MS);
  try {
    const res = await fetch(CRASH_ENDPOINT, {
      method: 'POST',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify(payload),
      signal: controller.signal,
    });
    if (!res.ok) throw new Error(`collector returned ${res.status}`);
    // Uploaded successfully — clear local copy so the next update doesn't
    // resend the same entries. User still gets a fresh buffer for the new
    // session's errors.
    clearDiagnostics();
    return true;
  } catch (e) {
    console.warn('[diagnostics] upload failed', e);
    return false;
  } finally {
    clearTimeout(timer);
  }
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
