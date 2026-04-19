# Fade crash reporter

A tiny Cloudflare Worker that proxies diagnostic reports from the Fade
desktop app into a GitHub `repository_dispatch` event. The Fade-App
workflow `.github/workflows/diag-report.yml` receives the dispatch and
opens an issue containing the payload.

## Why a proxy?

`repository_dispatch` requires a GitHub token with write access. That
token cannot be shipped in the desktop client — it would be extracted
and abused. The Worker holds the token server-side and validates that
incoming requests look like real diagnostic payloads before forwarding.

## One-time setup

```bash
# from this directory
npm i -g wrangler

# create a fine-grained PAT at https://github.com/settings/tokens?type=beta
#   Repository access: Only Fade-App
#   Permissions → Repository → Contents: Read and write
#   (repository_dispatch requires Contents write)

wrangler login
wrangler secret put GH_TOKEN   # paste the PAT
wrangler secret put GH_OWNER   # eldo9000
wrangler secret put GH_REPO    # Fade-App
wrangler deploy
```

Note the deployed URL (e.g.
`https://fade-crash-reporter.<account>.workers.dev`). Paste it into
`src/lib/stores/diagnostics.svelte.js` as `CRASH_ENDPOINT` (append
`/report`).

## Verifying

```bash
# health check — should return 200 with {"ok":true,...}
curl https://fade-crash-reporter.<account>.workers.dev

# simulated report — should return 204 and open an issue on Fade-App
curl -X POST https://fade-crash-reporter.<account>.workers.dev/report \
  -H 'content-type: application/json' \
  -d '{"version":"0.0.0-test","beta":true,"userAgent":"curl","entries":[{"t":'"$(date +%s)"'000,"source":"test","message":"hello from curl"}]}'
```

## Rotating the PAT

Generate a new PAT, run `wrangler secret put GH_TOKEN`, revoke the
old one. The Worker picks up the new secret on next invocation — no
redeploy needed.

## What the Worker does NOT do

- No rate limiting beyond the 64KB body cap. Cloudflare's free tier
  covers abuse up to the request-count limit; if someone starts
  hammering the endpoint we'd add a KV-based per-IP limiter here.
- No retention or storage — everything goes straight to GitHub and
  the Worker keeps no logs beyond Cloudflare's standard request logs.
- No authentication. The endpoint is public by design (no token to
  leak), and the worst a bad actor can do is open junk issues on
  Fade-App, which we can delete and filter by label.
