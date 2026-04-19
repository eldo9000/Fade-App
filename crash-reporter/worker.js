// Fade crash-reporter proxy — Cloudflare Worker.
//
// Accepts POST /report from the Fade client, validates it, and forwards the
// payload to GitHub's repository_dispatch API on behalf of a server-side PAT.
// A workflow in the Fade-App repo receives the dispatch and opens an issue.
//
// This Worker is the ONLY place the GitHub token lives — it is never shipped
// in the client. Keep the endpoint URL public (it's fine — the worst someone
// can do with it is submit bogus diagnostics), but rotate GH_TOKEN if the
// Worker code itself ever leaks.
//
// Secrets required (set via `wrangler secret put <NAME>`):
//   GH_TOKEN  — fine-grained PAT, repo-scoped to Fade-App, with
//               "Contents: read and write" permission (required for
//               repository_dispatch).
//   GH_OWNER  — e.g. "eldo9000"
//   GH_REPO   — e.g. "Fade-App"

const MAX_BODY_BYTES = 64 * 1024; // ≈100-entry ring buffer fits comfortably

export default {
  async fetch(req, env) {
    if (req.method === 'GET') {
      // Health check — no side effects.
      return json({ ok: true, service: 'fade-crash-reporter' });
    }
    if (req.method !== 'POST') {
      return new Response('Method not allowed', { status: 405 });
    }

    const url = new URL(req.url);
    if (url.pathname !== '/report') {
      return new Response('Not found', { status: 404 });
    }

    const contentLength = Number(req.headers.get('content-length') || '0');
    if (contentLength > MAX_BODY_BYTES) {
      return new Response('Payload too large', { status: 413 });
    }

    const body = await req.text();
    if (body.length > MAX_BODY_BYTES) {
      return new Response('Payload too large', { status: 413 });
    }

    let payload;
    try {
      payload = JSON.parse(body);
    } catch {
      return new Response('Bad JSON', { status: 400 });
    }

    if (!payload || typeof payload !== 'object' || !Array.isArray(payload.entries)) {
      return new Response('Missing entries', { status: 400 });
    }

    const ghRes = await fetch(
      `https://api.github.com/repos/${env.GH_OWNER}/${env.GH_REPO}/dispatches`,
      {
        method: 'POST',
        headers: {
          'authorization': `Bearer ${env.GH_TOKEN}`,
          'accept': 'application/vnd.github+json',
          'user-agent': 'fade-crash-reporter/1',
          'content-type': 'application/json',
        },
        body: JSON.stringify({
          event_type: 'diag-report',
          client_payload: payload,
        }),
      },
    );

    if (!ghRes.ok) {
      const text = await ghRes.text();
      return new Response(`GitHub dispatch failed: ${ghRes.status} ${text}`, {
        status: 502,
      });
    }
    return new Response(null, { status: 204 });
  },
};

function json(obj) {
  return new Response(JSON.stringify(obj), {
    headers: { 'content-type': 'application/json' },
  });
}
