<script>
  import { invoke } from '@tauri-apps/api/core';
  import { listen } from '@tauri-apps/api/event';
  import { onMount, onDestroy } from 'svelte';
  import { WindowFrame, Titlebar, TabBar, ProgressBar, Dialog } from '@libre/ui';
  import Queue from './lib/Queue.svelte';
  import ImageOptions from './lib/ImageOptions.svelte';
  import VideoOptions from './lib/VideoOptions.svelte';
  import AudioOptions from './lib/AudioOptions.svelte';
  import { mediaTypeFor, validateOptions, formatBytes, formatDuration } from './lib/utils.js';

  // ── State ──────────────────────────────────────────────────────────────────

  let queue = $state([]);
  let activeTab = $state('image'); // 'image' | 'video' | 'audio'
  let converting = $state(false);
  let paused = $state(false);
  let overallPercent = $state(0);
  let statusMessage = $state('');
  let validationErrors = $state({});
  let toolWarnings = $state({}); // { ffmpeg: false, ffprobe: false, magick: false }

  // File info dialog
  let fileInfoOpen = $state(false);
  let fileInfoData = $state(null);
  let fileInfoItem = $state(null);
  let fileInfoLoading = $state(false);

  let imageOptions = $state({
    output_format: 'webp',
    resize_mode: 'none',
    resize_percent: 100,
    resize_width: 1920,
    resize_height: 1080,
    quality: 85,
    crop_x: 0,
    crop_y: 0,
    crop_width: null,
    crop_height: null,
    rotation: 0,
    flip_h: false,
    flip_v: false,
    auto_rotate: true,
    output_dir: null,
  });

  let videoOptions = $state({
    output_format: 'mp4',
    codec: 'h264',
    resolution: 'original',
    trim_start: null,
    trim_end: null,
    remove_audio: false,
    extract_audio: false,
    audio_format: 'mp3',
    bitrate: 192,
    sample_rate: 48000,
    output_dir: null,
  });

  let audioOptions = $state({
    output_format: 'mp3',
    bitrate: 192,
    sample_rate: 44100,
    normalize_loudness: false,
    trim_start: null,
    trim_end: null,
    output_dir: null,
  });

  // ── Event listeners ────────────────────────────────────────────────────────

  let unlistenProgress, unlistenDone, unlistenError, unlistenCancelled;

  onMount(async () => {
    unlistenProgress = await listen('job-progress', ({ payload }) => {
      const item = queue.find(q => q.id === payload.job_id);
      if (item) {
        item.status = 'converting';
        item.percent = payload.percent;
        statusMessage = payload.message;
      }
      updateOverall();
    });

    unlistenDone = await listen('job-done', ({ payload }) => {
      const item = queue.find(q => q.id === payload.job_id);
      if (item) {
        item.status = 'done';
        item.percent = 100;
        item.outputPath = payload.output_path;
      }
      updateOverall();
      checkAllDone();
    });

    unlistenError = await listen('job-error', ({ payload }) => {
      const item = queue.find(q => q.id === payload.job_id);
      if (item) {
        item.status = 'error';
        item.error = payload.message;
      }
      updateOverall();
      checkAllDone();
    });

    unlistenCancelled = await listen('job-cancelled', ({ payload }) => {
      const item = queue.find(q => q.id === payload.job_id);
      if (item) {
        item.status = 'cancelled';
        item.percent = 0;
      }
      updateOverall();
      checkAllDone();
    });

    loadPresets();
    checkTools();
  });

  onDestroy(() => {
    unlistenProgress?.();
    unlistenDone?.();
    unlistenError?.();
    unlistenCancelled?.();
  });

  // ── Tool detection ─────────────────────────────────────────────────────────

  async function checkTools() {
    try {
      const result = await invoke('check_tools');
      toolWarnings = {
        ffmpeg: !result.ffmpeg,
        ffprobe: !result.ffprobe,
        magick: !result.magick,
      };
    } catch { /* non-fatal */ }
  }

  let dismissedWarnings = $state(new Set());

  function dismissWarning(tool) {
    const next = new Set(dismissedWarnings);
    next.add(tool);
    dismissedWarnings = next;
  }

  // ── Helpers ────────────────────────────────────────────────────────────────

  function updateOverall() {
    if (queue.length === 0) { overallPercent = 0; return; }
    const total = queue.reduce((sum, q) => sum + (q.percent ?? 0), 0);
    overallPercent = total / queue.length;
  }

  function checkAllDone() {
    const active = queue.filter(q => q.status === 'converting' || q.status === 'pending');
    if (active.length === 0) {
      converting = false;
      paused = false;
      const done = queue.filter(q => q.status === 'done').length;
      const cancelled = queue.filter(q => q.status === 'cancelled').length;
      const errored = queue.filter(q => q.status === 'error').length;
      const parts = [];
      if (done) parts.push(`${done} converted`);
      if (cancelled) parts.push(`${cancelled} cancelled`);
      if (errored) parts.push(`${errored} failed`);
      statusMessage = parts.length ? `Done — ${parts.join(', ')}` : 'Done';
    }
  }

  async function addFiles(paths) {
    for (const path of paths) {
      const name = path.split('/').pop() ?? path;
      const ext = name.includes('.') ? name.split('.').pop().toLowerCase() : '';
      const mt = mediaTypeFor(ext);
      const id = crypto.randomUUID();
      queue.push({ id, path, name, ext, mediaType: mt, status: 'pending', percent: 0 });
    }
    const types = queue.map(q => q.mediaType);
    if (types.every(t => t === 'image')) activeTab = 'image';
    else if (types.every(t => t === 'video')) activeTab = 'video';
    else if (types.every(t => t === 'audio')) activeTab = 'audio';
  }

  function removeItem(id) {
    queue = queue.filter(q => q.id !== id);
    updateOverall();
  }

  function clearQueue() {
    queue = [];
    overallPercent = 0;
    statusMessage = '';
    converting = false;
    paused = false;
    validationErrors = {};
  }

  // ── Cancel / Pause ─────────────────────────────────────────────────────────

  async function cancelJob(id) {
    try {
      await invoke('cancel_job', { jobId: id });
    } catch (e) {
      console.error('cancel_job failed:', e);
    }
  }

  async function cancelAll() {
    const active = queue.filter(q => q.status === 'converting');
    for (const item of active) {
      await cancelJob(item.id);
    }
    // Mark pending items as cancelled too (they were queued but not yet dispatched)
    for (const item of queue) {
      if (item.status === 'pending') {
        item.status = 'cancelled';
      }
    }
    paused = false;
    checkAllDone();
  }

  function togglePause() {
    if (paused) {
      paused = false;
      startConvert();
    } else {
      paused = true;
      statusMessage = 'Paused — click Resume to continue';
    }
  }

  // ── File info dialog ───────────────────────────────────────────────────────

  async function showFileInfo(item) {
    fileInfoItem = item;
    fileInfoOpen = true;
    fileInfoData = null;
    fileInfoLoading = true;
    try {
      fileInfoData = await invoke('get_file_info', { path: item.path });
    } catch (e) {
      fileInfoData = { error: String(e) };
    } finally {
      fileInfoLoading = false;
    }
  }

  function estimatedOutputSize(info) {
    if (!info || info.error) return null;
    if (info.media_type === 'image') {
      const q = imageOptions.quality ?? 85;
      return Math.round(info.file_size * (q / 100));
    }
    if (info.media_type === 'video' && info.duration_secs) {
      const br = (videoOptions.bitrate ?? 192) * 1000 / 8;
      return Math.round(info.duration_secs * br);
    }
    if (info.media_type === 'audio' && info.duration_secs) {
      const br = (audioOptions.bitrate ?? 192) * 1000 / 8;
      return Math.round(info.duration_secs * br);
    }
    return null;
  }

  // ── Convert ────────────────────────────────────────────────────────────────

  async function startConvert() {
    // Validate before dispatching
    const errors = validateOptions(videoOptions, audioOptions);
    if (Object.keys(errors).length > 0) {
      validationErrors = errors;
      return;
    }
    validationErrors = {};

    const pending = queue.filter(q => q.status === 'pending' || q.status === 'error');
    if (pending.length === 0) return;

    converting = true;
    paused = false;
    statusMessage = 'Converting…';

    for (const item of pending) {
      if (paused) break; // pause between items (not mid-FFmpeg)

      item.status = 'converting';
      item.percent = 0;

      let opts;
      if (item.mediaType === 'image') {
        opts = { ...imageOptions, output_suffix: outputSuffix };
      } else if (item.mediaType === 'video') {
        opts = { ...videoOptions, output_suffix: outputSuffix };
      } else {
        opts = { ...audioOptions, output_suffix: outputSuffix };
      }

      invoke('convert_file', {
        jobId: item.id,
        inputPath: item.path,
        options: opts,
      }).catch(err => {
        item.status = 'error';
        item.error = String(err);
        checkAllDone();
      });
    }
  }

  // ── Drag over window ───────────────────────────────────────────────────────

  let outputSuffix = $state('converted');
  let dragOver = $state(false);

  function onWindowDragover(e) { e.preventDefault(); dragOver = true; }
  function onWindowDragleave(e) { if (!e.relatedTarget) dragOver = false; }
  function onWindowDrop(e) {
    e.preventDefault();
    dragOver = false;
    const paths = Array.from(e.dataTransfer?.files ?? []).map(f => f.path ?? f.name);
    if (paths.length) addFiles(paths);
  }

  // ── Presets ────────────────────────────────────────────────────────────────

  let presets = $state([]);
  let presetsOpen = $state(false);
  let presetSaving = $state(false);
  let presetNameInput = $state('');

  async function loadPresets() {
    try { presets = await invoke('list_presets'); } catch { /* no-op */ }
  }

  async function savePreset() {
    const name = presetNameInput.trim();
    if (!name) return;
    const tab = activeTab;
    let src = tab === 'image' ? imageOptions : tab === 'video' ? videoOptions : audioOptions;
    try {
      const saved = await invoke('save_preset', {
        name, mediaType: tab,
        outputFormat: src.output_format,
        quality: tab === 'image' ? src.quality : null,
        codec: tab === 'video' ? src.codec : null,
        bitrate: (tab === 'video' || tab === 'audio') ? src.bitrate : null,
        sampleRate: (tab === 'video' || tab === 'audio') ? src.sample_rate : null,
      });
      presets = [...presets, saved];
      presetNameInput = '';
      presetSaving = false;
    } catch (e) { console.error('Save preset failed:', e); }
  }

  async function deletePreset(id) {
    try {
      await invoke('delete_preset', { id });
      presets = presets.filter(p => p.id !== id);
    } catch (e) { console.error('Delete preset failed:', e); }
  }

  function loadPresetIntoOptions(preset) {
    if (preset.media_type === 'image') {
      imageOptions.output_format = preset.output_format;
      if (preset.quality != null) imageOptions.quality = preset.quality;
      activeTab = 'image';
    } else if (preset.media_type === 'video') {
      videoOptions.output_format = preset.output_format;
      if (preset.codec != null) videoOptions.codec = preset.codec;
      if (preset.bitrate != null) videoOptions.bitrate = preset.bitrate;
      if (preset.sample_rate != null) videoOptions.sample_rate = preset.sample_rate;
      activeTab = 'video';
    } else {
      audioOptions.output_format = preset.output_format;
      if (preset.bitrate != null) audioOptions.bitrate = preset.bitrate;
      if (preset.sample_rate != null) audioOptions.sample_rate = preset.sample_rate;
      activeTab = 'audio';
    }
    presetsOpen = false;
  }

  const mediaTabs = [
    { id: 'image', label: 'Image' },
    { id: 'video', label: 'Video' },
    { id: 'audio', label: 'Audio' },
  ];

</script>

<WindowFrame
  ondragover={onWindowDragover}
  ondragleave={onWindowDragleave}
  ondrop={onWindowDrop}
>
  <!-- Titlebar -->
  <Titlebar>
    <div class="flex items-center gap-2 pl-3 flex-1 min-w-0" data-tauri-drag-region>
      <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor"
           stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round"
           class="text-[var(--accent)] shrink-0">
        <path d="M14.5 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V7.5L14.5 2z"/>
        <polyline points="14 2 14 8 20 8"/>
        <line x1="9" y1="15" x2="15" y2="15"/>
        <line x1="12" y1="12" x2="12" y2="18"/>
      </svg>
      <span class="text-[13px] font-medium text-[var(--text-primary)] truncate">Fade</span>
    </div>
  </Titlebar>

  <!-- Body: queue sidebar + main panel -->
  <div class="flex flex-1 min-h-0">

    <!-- Left: Queue -->
    <aside class="w-64 shrink-0 border-r border-[var(--border)] flex flex-col bg-[var(--surface-raised)]"
           role="region" aria-label="File queue">
      <Queue
        {queue}
        onadd={(paths) => addFiles(paths)}
        onremove={(id) => removeItem(id)}
        onclear={clearQueue}
        oncancel={(id) => cancelJob(id)}
        oninfo={(item) => showFileInfo(item)}
      />
    </aside>

    <!-- Right: options + action bar -->
    <div class="flex flex-col flex-1 min-w-0">

      <!-- Tab bar -->
      <TabBar tabs={mediaTabs} active={activeTab} onSelect={(id) => activeTab = id} />

      <!-- Tool warning banners -->
      {#if toolWarnings.ffmpeg && !dismissedWarnings.has('ffmpeg')}
        <div class="flex items-center justify-between gap-2 px-4 py-2
                    bg-amber-50 dark:bg-amber-900/20 border-b border-amber-200 dark:border-amber-800
                    text-[12px] text-amber-800 dark:text-amber-200 shrink-0">
          <span>FFmpeg not found — install <code class="font-mono">ffmpeg</code> to convert video and audio</span>
          <button onclick={() => dismissWarning('ffmpeg')}
                  class="shrink-0 text-amber-600 hover:text-amber-800 dark:hover:text-amber-100"
                  aria-label="Dismiss">×</button>
        </div>
      {/if}
      {#if toolWarnings.magick && !dismissedWarnings.has('magick')}
        <div class="flex items-center justify-between gap-2 px-4 py-2
                    bg-amber-50 dark:bg-amber-900/20 border-b border-amber-200 dark:border-amber-800
                    text-[12px] text-amber-800 dark:text-amber-200 shrink-0">
          <span>ImageMagick not found — install <code class="font-mono">imagemagick</code> to convert images</span>
          <button onclick={() => dismissWarning('magick')}
                  class="shrink-0 text-amber-600 hover:text-amber-800 dark:hover:text-amber-100"
                  aria-label="Dismiss">×</button>
        </div>
      {/if}

      <!-- Options panel -->
      <div class="flex-1 min-h-0 overflow-y-auto p-5" role="tabpanel">
        {#if activeTab === 'image'}
          <ImageOptions bind:options={imageOptions} />
        {:else if activeTab === 'video'}
          <VideoOptions bind:options={videoOptions} errors={validationErrors} />
        {:else}
          <AudioOptions bind:options={audioOptions} errors={validationErrors} />
        {/if}
      </div>

      <!-- Presets popover -->
      {#if presetsOpen}
        <div class="fixed inset-0 z-30" aria-hidden="true" onclick={() => { presetsOpen = false; presetSaving = false; }}></div>
        <div class="absolute bottom-[52px] right-4 z-40 w-64
                    bg-[var(--surface-raised)] border border-[var(--border)]
                    rounded-lg shadow-lg overflow-hidden">
          {#each presets.filter(p => p.media_type === activeTab) as p (p.id)}
            <div class="flex items-center gap-1 px-3 py-1.5 hover:bg-[var(--surface)] group">
              <button
                onclick={() => loadPresetIntoOptions(p)}
                class="flex-1 text-left text-[12px] text-[var(--text-primary)] truncate"
                title="Load preset: {p.name}"
              >{p.name}</button>
              <button
                onclick={() => deletePreset(p.id)}
                class="shrink-0 w-5 h-5 flex items-center justify-center rounded
                       text-[var(--text-secondary)] opacity-0 group-hover:opacity-100
                       hover:text-red-500 transition-all text-[13px]"
                aria-label="Delete preset {p.name}"
              >×</button>
            </div>
          {:else}
            <p class="px-3 py-2 text-[12px] text-[var(--text-secondary)]">No {activeTab} presets yet</p>
          {/each}

          <div class="border-t border-[var(--border)]">
            {#if presetSaving}
              <div class="flex items-center gap-1.5 px-3 py-2">
                <!-- svelte-ignore a11y_autofocus -->
                <input
                  type="text"
                  bind:value={presetNameInput}
                  placeholder="Preset name"
                  aria-label="Preset name"
                  autofocus
                  onkeydown={(e) => { if (e.key === 'Enter') savePreset(); if (e.key === 'Escape') { presetSaving = false; presetNameInput = ''; } }}
                  class="flex-1 px-2 py-1 text-[12px] rounded border border-[var(--border)]
                         bg-[var(--surface)] text-[var(--text-primary)] outline-none
                         focus:border-[var(--accent)] transition-colors"
                />
                <button
                  onclick={savePreset}
                  class="px-2 py-1 text-[12px] rounded bg-[var(--accent)] text-white
                         hover:opacity-90 transition-opacity shrink-0"
                >Save</button>
              </div>
            {:else}
              <button
                onclick={() => { presetSaving = true; presetNameInput = ''; }}
                class="w-full text-left px-3 py-2 text-[12px] text-[var(--accent)]
                       hover:bg-[var(--surface)] transition-colors"
              >+ Save current as preset…</button>
            {/if}
          </div>
        </div>
      {/if}

      <!-- Action bar -->
      <div class="relative shrink-0 border-t border-[var(--border)] px-5 py-3 flex items-center gap-4
                  bg-[var(--surface-raised)]">

        <!-- Status + progress -->
        <div class="flex-1 min-w-0">
          <div aria-live="polite" aria-atomic="true">
            {#if statusMessage}
              <p class="text-[12px] text-[var(--text-secondary)] truncate mb-1">{statusMessage}</p>
            {/if}
          </div>
          {#if converting || overallPercent > 0}
            <ProgressBar value={overallPercent} />
          {/if}
        </div>

        <!-- Suffix input -->
        <div class="flex items-center gap-1.5 shrink-0">
          <label for="output-suffix" class="text-[12px] text-[var(--text-secondary)] whitespace-nowrap">
            Suffix
          </label>
          <input
            id="output-suffix"
            type="text"
            bind:value={outputSuffix}
            disabled={converting}
            placeholder="converted"
            class="w-28 px-2 py-1 text-[12px] rounded border border-[var(--border)]
                   bg-[var(--surface)] text-[var(--text-primary)] outline-none
                   focus:border-[var(--accent)] transition-colors
                   disabled:opacity-50 font-mono"
            aria-label="Output filename suffix"
          />
        </div>

        <!-- Presets button -->
        <button
          onclick={() => { presetsOpen = !presetsOpen; presetSaving = false; }}
          class="px-3 py-1.5 rounded-md text-[13px] font-medium transition-colors shrink-0
                 border border-[var(--border)] text-[var(--text-secondary)]
                 hover:text-[var(--text-primary)] hover:border-[var(--accent)]
                 {presetsOpen ? 'border-[var(--accent)] text-[var(--accent)]' : ''}"
          aria-label="Presets"
        >Presets</button>

        <!-- Pause/Resume button (visible when converting) -->
        {#if converting}
          <button
            onclick={togglePause}
            class="px-3 py-1.5 rounded-md text-[13px] font-medium transition-colors shrink-0
                   border border-[var(--border)] text-[var(--text-secondary)]
                   hover:text-[var(--text-primary)] hover:border-[var(--accent)]"
          >{paused ? 'Resume' : 'Pause'}</button>

          <!-- Cancel All button -->
          <button
            onclick={cancelAll}
            class="px-3 py-1.5 rounded-md text-[13px] font-medium transition-colors shrink-0
                   border border-red-300 text-red-500
                   hover:border-red-500 hover:bg-red-50 dark:hover:bg-red-900/20"
          >Cancel All</button>
        {/if}

        <!-- Convert button -->
        <button
          onclick={startConvert}
          disabled={converting || queue.length === 0}
          class="px-5 py-1.5 rounded-md text-[13px] font-medium transition-colors shrink-0
            {converting || queue.length === 0
              ? 'bg-[var(--border)] text-[var(--text-secondary)] cursor-not-allowed'
              : 'bg-[var(--accent)] text-white hover:bg-[var(--accent-hover)]'}"
        >
          {converting ? 'Converting…' : 'Convert'}
        </button>
      </div>

    </div>
  </div>

  <!-- Full-window drag overlay -->
  {#if dragOver}
    <div class="absolute inset-0 z-40 flex items-center justify-center
                bg-[var(--accent)]/10 border-2 border-dashed border-[var(--accent)]
                pointer-events-none rounded-sm">
      <p class="text-[var(--accent)] text-lg font-medium">Drop files to add</p>
    </div>
  {/if}

  <!-- File info dialog -->
  <Dialog
    bind:open={fileInfoOpen}
    title={fileInfoItem ? fileInfoItem.name : 'File Info'}
    size="sm"
    onclose={() => { fileInfoOpen = false; fileInfoData = null; fileInfoItem = null; }}
  >
    {#if fileInfoLoading}
      <p class="text-[var(--text-secondary)] text-[13px]">Loading…</p>
    {:else if fileInfoData?.error}
      <p class="text-red-500 text-[12px]">{fileInfoData.error}</p>
    {:else if fileInfoData}
      {@const estSize = estimatedOutputSize(fileInfoData)}
      <dl class="space-y-2 text-[13px]">
        <div class="flex justify-between">
          <dt class="text-[var(--text-secondary)]">Format</dt>
          <dd class="text-[var(--text-primary)] font-mono uppercase">{fileInfoData.format ?? fileInfoData.media_type}</dd>
        </div>
        {#if fileInfoData.codec}
          <div class="flex justify-between">
            <dt class="text-[var(--text-secondary)]">Codec</dt>
            <dd class="text-[var(--text-primary)] font-mono">{fileInfoData.codec}</dd>
          </div>
        {/if}
        {#if fileInfoData.width && fileInfoData.height}
          <div class="flex justify-between">
            <dt class="text-[var(--text-secondary)]">Resolution</dt>
            <dd class="text-[var(--text-primary)] font-mono">{fileInfoData.width}×{fileInfoData.height}</dd>
          </div>
        {/if}
        {#if fileInfoData.duration_secs}
          <div class="flex justify-between">
            <dt class="text-[var(--text-secondary)]">Duration</dt>
            <dd class="text-[var(--text-primary)] font-mono">{formatDuration(fileInfoData.duration_secs)}</dd>
          </div>
        {/if}
        <div class="flex justify-between">
          <dt class="text-[var(--text-secondary)]">File size</dt>
          <dd class="text-[var(--text-primary)] font-mono">{formatBytes(fileInfoData.file_size)}</dd>
        </div>
        {#if estSize}
          <div class="flex justify-between border-t border-[var(--border)] pt-2 mt-2">
            <dt class="text-[var(--text-secondary)]">Est. output size</dt>
            <dd class="text-[var(--text-primary)] font-mono">{formatBytes(estSize)} <span class="text-[11px] text-[var(--text-secondary)]">(approx)</span></dd>
          </div>
        {/if}
      </dl>
    {/if}
  </Dialog>

</WindowFrame>
