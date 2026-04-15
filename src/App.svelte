<script>
  import { invoke, convertFileSrc } from '@tauri-apps/api/core';
  import { listen } from '@tauri-apps/api/event';
  import { onMount, onDestroy } from 'svelte';
  import { initTheme } from '@libre/ui/src/theme.js';
  import { ProgressBar, Dialog } from '@libre/ui';
  import Queue from './lib/Queue.svelte';
  import Timeline from './lib/Timeline.svelte';
  import ImageOptions from './lib/ImageOptions.svelte';
  import VideoOptions from './lib/VideoOptions.svelte';
  import AudioOptions from './lib/AudioOptions.svelte';
  import DataOptions from './lib/DataOptions.svelte';
  import DocumentOptions from './lib/DocumentOptions.svelte';
  import ArchiveOptions from './lib/ArchiveOptions.svelte';
  import { mediaTypeFor, validateOptions, formatBytes, formatDuration } from './lib/utils.js';

  // ── State ──────────────────────────────────────────────────────────────────

  let queue = $state([]);
  let selectedId = $state(null);
  let selectedItem = $derived(queue.find(q => q.id === selectedId) ?? null);

  let converting = $state(false);
  let paused = $state(false);
  let overallPercent = $state(0);
  let statusMessage = $state('');
  let validationErrors = $state({});
  let toolWarnings = $state({});

  // Auto-load duration when a video/audio item is selected
  let selectedDuration = $state(null);
  $effect(() => {
    const item = selectedItem;
    if (!item || (item.mediaType !== 'video' && item.mediaType !== 'audio')) {
      selectedDuration = null;
      return;
    }
    invoke('get_file_info', { path: item.path })
      .then(info => { selectedDuration = info.duration_secs ?? null; })
      .catch(() => { selectedDuration = null; });
  });

  // File info dialog
  let fileInfoOpen = $state(false);
  let fileInfoData = $state(null);
  let fileInfoItem = $state(null);
  let fileInfoLoading = $state(false);

  // Browse file input
  let fileInput = $state(null);

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
    dsp_highpass_freq: null,
    dsp_lowpass_freq: null,
    dsp_stereo_width: null,
    dsp_limiter_db: null,
    output_dir: null,
  });

  let dataOptions = $state({
    output_format: 'json',
    pretty_print: true,
    csv_delimiter: ',',
    output_dir: null,
  });

  let documentOptions = $state({
    output_format: 'html',
    output_dir: null,
  });

  let archiveOptions = $state({
    output_format: 'zip',
    archive_operation: 'convert',
    output_dir: null,
  });

  // ── Event listeners ────────────────────────────────────────────────────────

  let unlistenProgress, unlistenDone, unlistenError, unlistenCancelled;

  onMount(async () => {
    await initTheme(invoke);

    // Pre-load Test-Files folder
    try {
      const testDir = '/Users/eldo/Desktop/Test-Files';
      const files = await invoke('scan_dir', { path: testDir });
      if (files.length > 0) {
        addFiles(files);
        imageOptions.output_dir = testDir;
        videoOptions.output_dir = testDir;
        audioOptions.output_dir = testDir;
        dataOptions.output_dir = testDir;
        documentOptions.output_dir = testDir;
        archiveOptions.output_dir = testDir;
      }
    } catch { /* non-fatal — folder may not exist */ }

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
        sevenzip: !result.sevenzip,
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

  function addFiles(paths) {
    let firstNewId = null;
    for (const path of paths) {
      const name = path.split('/').pop() ?? path;
      const ext = name.includes('.') ? name.split('.').pop().toLowerCase() : '';
      const mt = mediaTypeFor(ext);
      const id = crypto.randomUUID();
      if (!firstNewId) firstNewId = id;
      queue.push({ id, path, name, ext, mediaType: mt, status: 'pending', percent: 0 });
    }
    if (!selectedId && firstNewId) selectedId = firstNewId;
  }

  function removeItem(id) {
    queue = queue.filter(q => q.id !== id);
    if (selectedId === id) selectedId = queue.length > 0 ? queue[0].id : null;
    updateOverall();
  }

  function clearQueue() {
    queue = [];
    selectedId = null;
    overallPercent = 0;
    statusMessage = '';
    converting = false;
    paused = false;
    validationErrors = {};
  }

  // ── Browse ─────────────────────────────────────────────────────────────────

  function onBrowse() { fileInput?.click(); }

  function onFileInputChange(e) {
    const paths = Array.from(e.target.files ?? []).map(f => f.path ?? f.name);
    if (paths.length) addFiles(paths);
    e.target.value = '';
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
    for (const item of active) await cancelJob(item.id);
    for (const item of queue) {
      if (item.status === 'pending') item.status = 'cancelled';
    }
    paused = false;
    checkAllDone();
  }

  function togglePause() {
    if (paused) { paused = false; startConvert(); }
    else { paused = true; statusMessage = 'Paused — click Resume to continue'; }
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
      return Math.round(info.file_size * ((imageOptions.quality ?? 85) / 100));
    }
    if (info.media_type === 'video' && info.duration_secs) {
      return Math.round(info.duration_secs * (videoOptions.bitrate ?? 192) * 1000 / 8);
    }
    if (info.media_type === 'audio' && info.duration_secs) {
      return Math.round(info.duration_secs * (audioOptions.bitrate ?? 192) * 1000 / 8);
    }
    return null;
  }

  // ── Convert ────────────────────────────────────────────────────────────────

  async function startConvert() {
    const errors = validateOptions(videoOptions, audioOptions);
    if (Object.keys(errors).length > 0) { validationErrors = errors; return; }
    validationErrors = {};

    const pending = queue.filter(q => q.status === 'pending' || q.status === 'error');
    if (pending.length === 0) return;

    converting = true;
    paused = false;
    statusMessage = 'Converting…';

    for (const item of pending) {
      if (paused) break;
      item.status = 'converting';
      item.percent = 0;

      const opts = item.mediaType === 'image'    ? { ...imageOptions,    output_suffix: outputSuffix }
             : item.mediaType === 'video'    ? { ...videoOptions,    output_suffix: outputSuffix }
             : item.mediaType === 'audio'    ? { ...audioOptions,    output_suffix: outputSuffix }
             : item.mediaType === 'data'     ? { ...dataOptions,     output_suffix: outputSuffix }
             : item.mediaType === 'document' ? { ...documentOptions, output_suffix: outputSuffix }
             :                                 { ...archiveOptions,   output_suffix: outputSuffix };

      invoke('convert_file', { jobId: item.id, inputPath: item.path, options: opts })
        .catch(err => { item.status = 'error'; item.error = String(err); checkAllDone(); });
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
    const tab = selectedItem?.mediaType ?? 'image';
    const src = tab === 'image' ? imageOptions : tab === 'video' ? videoOptions : audioOptions;
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
    } else if (preset.media_type === 'video') {
      videoOptions.output_format = preset.output_format;
      if (preset.codec != null) videoOptions.codec = preset.codec;
      if (preset.bitrate != null) videoOptions.bitrate = preset.bitrate;
      if (preset.sample_rate != null) videoOptions.sample_rate = preset.sample_rate;
    } else {
      audioOptions.output_format = preset.output_format;
      if (preset.bitrate != null) audioOptions.bitrate = preset.bitrate;
      if (preset.sample_rate != null) audioOptions.sample_rate = preset.sample_rate;
    }
    presetsOpen = false;
  }

  // Current media type for preset filtering etc.
  let activeMediaType = $derived(selectedItem?.mediaType ?? null);

  // Asset URL for the preview pane (video / image only)
  let previewSrc = $derived(
    selectedItem && (selectedItem.mediaType === 'video' || selectedItem.mediaType === 'image')
      ? convertFileSrc(selectedItem.path)
      : null
  );
</script>

<!-- Hidden file input for Browse button -->
<input
  type="file"
  multiple
  bind:this={fileInput}
  onchange={onFileInputChange}
  class="hidden"
  aria-hidden="true"
/>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="relative flex h-full bg-[var(--surface)] overflow-hidden select-none"
     ondragover={onWindowDragover}
     ondragleave={onWindowDragleave}
     ondrop={onWindowDrop}>

  <!-- ── 3-column body (full height, no titlebar) ───────────────────────────── -->

    <!-- ── LEFT: File queue (312px) ───────────────────────────────────────── -->
    <aside class="w-[312px] shrink-0 border-r border-[var(--border)] flex flex-col bg-[var(--surface-raised)]"
           role="region" aria-label="File queue">

      <!-- Queue header — pl-20 clears macOS traffic lights -->
      <div class="flex items-center gap-1.5 pl-20 pr-3 py-2.5 border-b border-[var(--border)] shrink-0"
           data-tauri-drag-region>
        <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor"
             stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round"
             class="text-[var(--accent)] shrink-0">
          <path d="M14.5 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V7.5L14.5 2z"/>
          <polyline points="14 2 14 8 20 8"/>
          <line x1="9" y1="15" x2="15" y2="15"/>
          <line x1="12" y1="12" x2="12" y2="18"/>
        </svg>
        <span class="text-[12px] font-semibold text-[var(--text-primary)] shrink-0">Fade</span>
        <div class="w-px h-3 bg-[var(--border)] mx-0.5 shrink-0"></div>
        <button
          onclick={onBrowse}
          class="px-2 py-0.5 rounded text-[11px] font-medium bg-[var(--accent)] text-white
                 hover:opacity-90 transition-opacity shrink-0"
        >Browse…</button>
        {#if queue.length > 0}
          <button
            onclick={clearQueue}
            class="px-2 py-0.5 rounded text-[11px] text-[var(--text-secondary)]
                   border border-[var(--border)] hover:text-red-400 hover:border-red-400
                   transition-colors shrink-0"
          >Clear</button>
        {/if}
      </div>

      <!-- Tool warnings -->
      {#if toolWarnings.ffmpeg && !dismissedWarnings.has('ffmpeg')}
        <div class="flex items-center justify-between gap-2 px-3 py-1.5
                    bg-amber-900/20 border-b border-amber-800
                    text-[11px] text-amber-200 shrink-0">
          <span>ffmpeg not found</span>
          <button onclick={() => dismissWarning('ffmpeg')} class="text-amber-400 hover:text-amber-200">×</button>
        </div>
      {/if}
      {#if toolWarnings.magick && !dismissedWarnings.has('magick')}
        <div class="flex items-center justify-between gap-2 px-3 py-1.5
                    bg-amber-900/20 border-b border-amber-800
                    text-[11px] text-amber-200 shrink-0">
          <span>ImageMagick not found</span>
          <button onclick={() => dismissWarning('magick')} class="text-amber-400 hover:text-amber-200">×</button>
        </div>
      {/if}

      <!-- File list -->
      <Queue
        {queue}
        {selectedId}
        onselect={(id) => selectedId = id}
        onremove={(id) => removeItem(id)}
        oncancel={(id) => cancelJob(id)}
        oninfo={(item) => showFileInfo(item)}
      />

      <!-- ── Sidebar bottom: output controls + convert ─────────────────────── -->
      <div class="shrink-0 border-t border-[var(--border)] flex flex-col gap-2 p-3">

        <!-- Output suffix -->
        <div class="flex items-center gap-2">
          <label for="output-suffix" class="text-[11px] text-[var(--text-secondary)] whitespace-nowrap w-12 shrink-0">
            Suffix
          </label>
          <input
            id="output-suffix"
            type="text"
            bind:value={outputSuffix}
            disabled={converting}
            placeholder="converted"
            class="flex-1 min-w-0 px-2 py-1 text-[12px] rounded border border-[var(--border)]
                   bg-[var(--surface)] text-[var(--text-primary)] outline-none
                   focus:border-[var(--accent)] transition-colors disabled:opacity-40 font-mono"
          />
          <button
            onclick={() => { presetsOpen = !presetsOpen; presetSaving = false; }}
            disabled={!activeMediaType}
            class="px-2 py-1 rounded text-[11px] border border-[var(--border)]
                   text-[var(--text-secondary)] hover:text-[var(--accent)] hover:border-[var(--accent)]
                   transition-colors disabled:opacity-40 disabled:cursor-not-allowed shrink-0
                   {presetsOpen ? 'text-[var(--accent)] border-[var(--accent)]' : ''}"
          >Presets</button>
        </div>

        <!-- Progress -->
        {#if statusMessage}
          <p class="text-[11px] text-[var(--text-secondary)] truncate" aria-live="polite">{statusMessage}</p>
        {/if}
        {#if converting || overallPercent > 0}
          <ProgressBar value={overallPercent} />
        {/if}

        <!-- Convert / Pause / Cancel -->
        <div class="flex gap-1.5">
          {#if converting}
            <button onclick={togglePause}
                    class="flex-1 py-1.5 rounded text-[12px] font-medium border border-[var(--border)]
                           text-[var(--text-secondary)] hover:text-[var(--text-primary)]
                           hover:border-[var(--accent)] transition-colors">
              {paused ? 'Resume' : 'Pause'}
            </button>
            <button onclick={cancelAll}
                    class="flex-1 py-1.5 rounded text-[12px] font-medium border border-red-800
                           text-red-400 hover:border-red-500 hover:bg-red-900/20 transition-colors">
              Cancel
            </button>
          {:else}
            <button
              onclick={startConvert}
              disabled={converting || queue.length === 0}
              class="flex-1 py-1.5 rounded text-[12px] font-semibold transition-colors
                     {queue.length === 0
                       ? 'bg-[var(--border)] text-[var(--text-secondary)] cursor-not-allowed'
                       : 'bg-[var(--accent)] text-white hover:opacity-90'}"
            >Convert</button>
          {/if}
        </div>
      </div>
    </aside>

    <!-- ── CENTER: Preview + timeline ─────────────────────────────────────── -->
    <div class="flex flex-col flex-1 min-w-0">

      <!-- Preview area -->
      <div class="flex-1 min-h-0 bg-[#0d0d0d] flex items-center justify-center relative overflow-hidden">
        {#key selectedId}
          {#if selectedItem?.mediaType === 'video' && previewSrc}
            <!-- svelte-ignore a11y_media_has_caption -->
            <video
              src={previewSrc}
              controls
              class="max-w-full max-h-full object-contain"
            ></video>
          {:else if selectedItem?.mediaType === 'image' && previewSrc}
            <img
              src={previewSrc}
              alt={selectedItem.name}
              class="max-w-full max-h-full object-contain"
            />
          {:else if selectedItem}
            <div class="text-center select-none">
              <p class="text-white/20 text-[11px] font-mono uppercase tracking-widest mb-2">
                {selectedItem.ext}
              </p>
              <p class="text-white/40 text-[13px] truncate max-w-xs px-4">
                {selectedItem.name}
              </p>
            </div>
          {:else}
            <div class="text-center select-none">
              <svg width="36" height="36" viewBox="0 0 24 24" fill="none" stroke="currentColor"
                   stroke-width="1" stroke-linecap="round" stroke-linejoin="round"
                   class="text-white/10 mx-auto mb-3">
                <rect x="2" y="2" width="20" height="20" rx="2"/>
                <circle cx="8" cy="8" r="2"/>
                <polyline points="22,14 16,8 5,19"/>
              </svg>
              <p class="text-white/20 text-[12px]">No file selected</p>
            </div>
          {/if}
        {/key}
      </div>

      <!-- Timeline -->
      {#if selectedItem?.mediaType === 'video'}
        <Timeline item={selectedItem} duration={selectedDuration} bind:options={videoOptions} />
      {:else if selectedItem?.mediaType === 'audio'}
        <Timeline item={selectedItem} duration={selectedDuration} bind:options={audioOptions} />
      {:else}
        <div class="h-28 shrink-0 border-t border-[var(--border)] flex items-center justify-center"
             style="background:#0a0a0a">
          <span class="text-[11px]" style="color:#333">—</span>
        </div>
      {/if}
    </div>

    <!-- ── RIGHT: Options panel (333px, adapts to selected file type) ──────── -->
    <aside class="w-[333px] shrink-0 border-l border-[var(--border)] flex flex-col bg-[var(--surface-raised)]"
           role="region" aria-label="Conversion options">

      <!-- Panel header -->
      <div class="flex items-center justify-between px-3 py-2 border-b border-[var(--border)] shrink-0">
        <span class="text-[11px] font-medium text-[var(--text-secondary)] uppercase tracking-wide">
          {#if activeMediaType === 'image'}Image
          {:else if activeMediaType === 'video'}Video
          {:else if activeMediaType === 'audio'}Audio
          {:else if activeMediaType === 'data'}Data
          {:else if activeMediaType === 'document'}Document
          {:else if activeMediaType === 'archive'}Archive
          {:else}Options
          {/if}
        </span>
        <!-- Presets button -->
        <button
          onclick={() => { presetsOpen = !presetsOpen; presetSaving = false; }}
          disabled={!activeMediaType}
          class="text-[11px] px-2 py-0.5 rounded border border-[var(--border)]
                 text-[var(--text-secondary)] hover:text-[var(--accent)] hover:border-[var(--accent)]
                 transition-colors disabled:opacity-40 disabled:cursor-not-allowed
                 {presetsOpen ? 'text-[var(--accent)] border-[var(--accent)]' : ''}"
        >Presets</button>
      </div>

      <!-- Options content -->
      <div class="flex-1 min-h-0 overflow-y-auto p-4">
        {#if !selectedItem}
          <div class="flex flex-col items-center justify-center h-full text-center gap-2">
            <p class="text-[12px] text-[var(--text-secondary)]">
              Select a file to see conversion options
            </p>
          </div>
        {:else if selectedItem.mediaType === 'image'}
          <ImageOptions bind:options={imageOptions} />
        {:else if selectedItem.mediaType === 'video'}
          <VideoOptions bind:options={videoOptions} errors={validationErrors} />
        {:else if selectedItem.mediaType === 'audio'}
          <AudioOptions bind:options={audioOptions} errors={validationErrors} />
        {:else if selectedItem.mediaType === 'data'}
          <DataOptions bind:options={dataOptions} />
        {:else if selectedItem.mediaType === 'document'}
          <DocumentOptions bind:options={documentOptions} />
        {:else if selectedItem.mediaType === 'archive'}
          <ArchiveOptions bind:options={archiveOptions} toolWarnings={toolWarnings} />
        {:else}
          <div class="flex flex-col items-center justify-center h-full text-center gap-2">
            <p class="text-[12px] text-[var(--text-secondary)]">
              Unsupported file type
            </p>
          </div>
        {/if}
      </div>

      <!-- Presets popover -->
      {#if presetsOpen && activeMediaType}
        <div class="fixed inset-0 z-30" aria-hidden="true"
             onclick={() => { presetsOpen = false; presetSaving = false; }}></div>
        <div class="absolute bottom-[52px] right-4 z-40 w-56
                    bg-[var(--surface-raised)] border border-[var(--border)]
                    rounded-lg shadow-lg overflow-hidden">
          {#each presets.filter(p => p.media_type === activeMediaType) as p (p.id)}
            <div class="flex items-center gap-1 px-3 py-1.5 hover:bg-[var(--surface)] group">
              <button
                onclick={() => loadPresetIntoOptions(p)}
                class="flex-1 text-left text-[12px] text-[var(--text-primary)] truncate"
              >{p.name}</button>
              <button
                onclick={() => deletePreset(p.id)}
                class="shrink-0 w-5 h-5 flex items-center justify-center rounded
                       text-[var(--text-secondary)] opacity-0 group-hover:opacity-100
                       hover:text-red-500 transition-all text-[13px]"
                aria-label="Delete preset"
              >×</button>
            </div>
          {:else}
            <p class="px-3 py-2 text-[12px] text-[var(--text-secondary)]">No presets yet</p>
          {/each}
          <div class="border-t border-[var(--border)]">
            {#if presetSaving}
              <div class="flex items-center gap-1.5 px-3 py-2">
                <!-- svelte-ignore a11y_autofocus -->
                <input
                  type="text"
                  bind:value={presetNameInput}
                  placeholder="Preset name"
                  autofocus
                  onkeydown={(e) => { if (e.key === 'Enter') savePreset(); if (e.key === 'Escape') { presetSaving = false; presetNameInput = ''; } }}
                  class="flex-1 px-2 py-1 text-[12px] rounded border border-[var(--border)]
                         bg-[var(--surface)] text-[var(--text-primary)] outline-none
                         focus:border-[var(--accent)] transition-colors"
                />
                <button onclick={savePreset}
                        class="px-2 py-1 text-[12px] rounded bg-[var(--accent)] text-white
                               hover:opacity-90 shrink-0">Save</button>
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
    </aside>

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
            <dd class="text-[var(--text-primary)] font-mono">
              {formatBytes(estSize)}
              <span class="text-[11px] text-[var(--text-secondary)]">(approx)</span>
            </dd>
          </div>
        {/if}
      </dl>
    {/if}
  </Dialog>

</div>
