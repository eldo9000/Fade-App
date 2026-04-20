<script>
  import { invoke } from '@tauri-apps/api/core';
  import { check as checkUpdate } from '@tauri-apps/plugin-updater';
  import { relaunch } from '@tauri-apps/plugin-process';
  import { pushError } from './stores/diagnostics.svelte.js';

  let {
    settings,
    setStatus,
    onUploadBeforeUpdate = null,
    updateState = $bindable('idle'),
  } = $props();

  const RELEASES_URL = 'https://github.com/eldo9000/Fade-App/releases/latest';
  const isManualUpdatePlatform = typeof navigator !== 'undefined'
    && /Mac|Windows/.test(navigator.userAgent);

  let updateVersion = $state('');
  let updateProgress = $state(0); // 0..100, downloading only
  let _pendingUpdate = null;

  const WEEK_MS = 7 * 24 * 60 * 60 * 1000;

  async function openReleasesPage() {
    await onUploadBeforeUpdate?.();
    try { await invoke('open_url', { url: RELEASES_URL }); }
    catch (e) {
      pushError('updater', 'Could not open the releases page', e);
      setStatus('Could not open the releases page — check your browser', 'error');
    }
  }

  export async function checkForUpdate() {
    try {
      const update = await checkUpdate();
      settings.lastUpdateCheck = Date.now();
      if (update) {
        _pendingUpdate = update;
        updateVersion = update.version ?? '';
        updateState = 'available';
      } else {
        updateState = 'idle';
      }
    } catch (e) {
      pushError('updater', 'Update check failed', e);
      updateState = 'idle';
    }
  }

  /** Launch-gated check — runs at most once per week. Manual "Update Now" bypasses. */
  export function maybeCheckForUpdate() {
    const last = Number(settings.lastUpdateCheck) || 0;
    if (Date.now() - last >= WEEK_MS) checkForUpdate();
  }

  async function installUpdate() {
    if (!_pendingUpdate) return;
    await onUploadBeforeUpdate?.();
    updateState = 'downloading';
    updateProgress = 0;
    let downloaded = 0;
    let contentLength = 0;
    try {
      await _pendingUpdate.downloadAndInstall((event) => {
        switch (event.event) {
          case 'Started':
            contentLength = event.data?.contentLength ?? 0;
            break;
          case 'Progress':
            downloaded += event.data?.chunkLength ?? 0;
            if (contentLength > 0) {
              updateProgress = Math.min(100, Math.round((downloaded / contentLength) * 100));
            }
            break;
          case 'Finished':
            updateProgress = 100;
            break;
        }
      });
      // Do NOT relaunch automatically — user decides when to restart.
      updateState = 'ready';
    } catch (e) {
      pushError('updater', 'Update install failed', e);
      setStatus('Update install failed — see Diagnostics in Settings', 'error');
      updateState = 'idle';
    }
  }

  async function restartNow() {
    try { await relaunch(); }
    catch (e) {
      pushError('updater', 'Relaunch failed', e);
      setStatus('Could not restart — quit and reopen manually', 'error');
    }
  }
</script>

<!-- Section: Updates -->
<div class="px-4 pt-4 pb-3 border-b border-[var(--border)]">
  <!-- Header row: "Updates" + inline progress bar filling the rest -->
  <div class="flex items-center gap-3 mb-3">
    <p class="text-[10px] font-semibold uppercase tracking-widest text-[var(--text-secondary)] shrink-0">Updates</p>
    <div class="flex-1 h-1.5 rounded-full overflow-hidden bg-[var(--border)]">
      <div class="h-full bg-[var(--accent)] transition-all duration-200"
           style="width:{updateState === 'downloading' ? updateProgress : (updateState === 'ready' ? 100 : (updateState === 'available' ? 100 : 0))}%"></div>
    </div>
    {#if updateState === 'downloading'}
      <span class="text-[10px] font-mono text-[var(--text-secondary)] shrink-0 tabular-nums">{updateProgress}%</span>
    {:else if updateState === 'available' || updateState === 'ready'}
      <span class="text-[10px] font-mono text-[var(--accent)] shrink-0">v{updateVersion}</span>
    {/if}
  </div>
  <!-- Left-justified checkboxes + stateful action button -->
  <div class="flex items-center gap-4">
    <label class="flex items-center gap-2 cursor-pointer">
      <input type="checkbox" bind:checked={settings.notifyUpdates}
             class="w-3.5 h-3.5 accent-[var(--accent)]" />
      <span class="text-[12px] text-[var(--text-primary)]">Notify</span>
    </label>
    {#if !isManualUpdatePlatform}
      <label class="flex items-center gap-2 cursor-pointer">
        <input type="checkbox" bind:checked={settings.autoUpdate}
               class="w-3.5 h-3.5 accent-[var(--accent)]" />
        <span class="text-[12px] text-[var(--text-primary)]">Auto-update</span>
      </label>
    {/if}
    {#if updateState === 'available' && isManualUpdatePlatform}
      <button onclick={openReleasesPage}
              title="Opens the GitHub releases page in your browser"
              class="ml-auto px-2.5 py-1 rounded text-[11px] font-semibold shrink-0
                     bg-[var(--accent)] text-white border border-[color-mix(in_srgb,var(--accent)_70%,#000)]
                     hover:opacity-90 transition-opacity">
        Download update
      </button>
    {:else if updateState === 'available'}
      <button onclick={installUpdate}
              class="ml-auto px-2.5 py-1 rounded text-[11px] font-semibold shrink-0
                     bg-[var(--accent)] text-white border border-[color-mix(in_srgb,var(--accent)_70%,#000)]
                     hover:opacity-90 transition-opacity">
        Update Now
      </button>
    {:else if updateState === 'downloading'}
      <button disabled
              class="ml-auto inline-flex items-center gap-1.5 px-2.5 py-1 rounded text-[11px] shrink-0
                     border border-[var(--border)] text-[var(--text-secondary)]
                     bg-transparent opacity-70 cursor-not-allowed">
        <svg class="animate-spin" width="11" height="11" viewBox="0 0 24 24" fill="none"
             stroke="currentColor" stroke-width="3" stroke-linecap="round">
          <path d="M21 12a9 9 0 1 1-6.219-8.56" />
        </svg>
        Updating
      </button>
    {:else if updateState === 'ready'}
      <button onclick={restartNow}
              class="ml-auto px-2.5 py-1 rounded text-[11px] font-semibold shrink-0
                     bg-[var(--accent)] text-white border border-[color-mix(in_srgb,var(--accent)_70%,#000)]
                     hover:opacity-90 transition-opacity">
        Restart now
      </button>
    {:else}
      <button onclick={checkForUpdate}
              class="ml-auto px-2.5 py-1 rounded text-[11px] border border-[var(--border)]
                     text-[var(--text-secondary)] hover:text-[var(--text-primary)]
                     hover:border-[var(--accent)] transition-colors shrink-0">
        Update Now
      </button>
    {/if}
  </div>
</div>
