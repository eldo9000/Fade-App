<script>
  import { invoke, convertFileSrc } from '@tauri-apps/api/core';
  import { listen } from '@tauri-apps/api/event';
  import { onMount, onDestroy } from 'svelte';
  import { fade } from 'svelte/transition';
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

  // ── Sequential load pipeline ────────────────────────────────────────────────
  //
  //  Click → handleSelect (synchronous, one Svelte batch):
  //    liveSrc = null, all gates false, selectedId = newId
  //    → browser paints: black preview + "Loading" + queue highlight
  //
  //  Then runLoadPipeline (async, each step awaits the previous):
  //    Step 1  (50ms yield)   — let browser paint the cleared state
  //    Step 2  get_file_info  — fast ffprobe metadata → selectedDuration
  //    Step 3  liveSrc        — set video/image src → browser starts decode
  //    Step 4  mediaReady     — Timeline creates Audio / connects to <video>
  //    Step 5  waveformReady  — get_waveform (ffmpeg, medium)
  //    Step 6  spectrogramReady — get_spectrogram (ffmpeg, heaviest)
  //
  //  Each step checks _generation to bail if a newer click has started.

  let selectedDuration    = $state(null);
  let previewLoading      = $state(false);
  let liveSrc             = $state(null);
  let tlMediaReady        = $state(false);
  let tlWaveformReady     = $state(false);
  let tlSpectrogramReady  = $state(false);
  let tlFilmstripReady    = $state(false);
  let _generation         = 0;

  function onPreviewLoaded() { previewLoading = false; }

  /** Yield to browser for one frame — returns a Promise that resolves after paint */
  function frameYield(ms = 50) { return new Promise(r => setTimeout(r, ms)); }

  async function runLoadPipeline(gen, newItem) {
    const isMedia = newItem && ['video', 'audio', 'image'].includes(newItem.mediaType);
    if (!newItem || !isMedia) return;

    const stale = () => _generation !== gen;
    const mt = newItem.mediaType;

    // ── Step 1: yield so the browser paints the cleared state ──
    await frameYield(50);
    if (stale()) return;

    // ── Step 2: get_file_info (ffprobe, fast) ──
    if (mt === 'video' || mt === 'audio') {
      try {
        const info = await invoke('get_file_info', { path: newItem.path });
        if (stale()) return;
        selectedDuration = info.duration_secs ?? null;
      } catch {
        if (stale()) return;
        selectedDuration = null;
      }
    }
    if (stale()) return;

    // ── Step 3: set liveSrc (video / image decode starts) ──
    if (mt === 'video' || mt === 'image') {
      liveSrc = convertFileSrc(newItem.path);
    } else {
      // Audio — no visual preview, clear loading
      previewLoading = false;
    }
    if (stale()) return;

    // ── Step 4: unlock Timeline media element ──
    await frameYield(0);  // one more yield so liveSrc paints before Audio setup
    if (stale()) return;
    tlMediaReady = true;
    if (stale()) return;

    // ── Step 5: unlock waveform (ffmpeg, medium cost) ──
    await frameYield(0);
    if (stale()) return;
    tlWaveformReady = true;
    if (stale()) return;

    // ── Step 6: unlock spectrogram (ffmpeg, heaviest) ──
    // Wait a small beat so waveform invoke dispatches before spectrogram starts
    await frameYield(100);
    if (stale()) return;
    tlSpectrogramReady = true;

    // ── Step 7: unlock filmstrip (ffmpeg, background, lowest priority) ──
    // Yield long enough that waveform has finished and the user can interact.
    await frameYield(800);
    if (stale()) return;
    tlFilmstripReady = true;
  }

  // File info dialog
  let fileInfoOpen = $state(false);
  let fileInfoData = $state(null);
  let fileInfoItem = $state(null);
  let fileInfoLoading = $state(false);

  // Browse file input
  let fileInput = $state(null);

  // Preview video element — bound so Timeline can drive it
  let videoEl = $state(null);

  // Advanced audio panel — persists across file switches
  let vizExpanded = $state(false);
  let queueCompact = $state(false);

  // Compression diff preview
  let diffClipPath   = $state(null);
  let diffLoading    = $state(false);
  let diffError      = $state(null);
  let diffNote       = $state(null);
  let diffHandleSecs = $state(3); // 1 = fast, 3 = accurate, 10 = AV1-safe

  // Mini scrubber state for the diff clip
  let diffVideoEl     = $state(null);
  let diffCurrentTime = $state(0);
  let diffDuration    = $state(0);
  let diffPlaying     = $state(false);
  let diffTrackEl     = $state(null);
  let diffDragging    = $state(false);

  function toggleDiffPlay() {
    if (!diffVideoEl) return;
    if (diffPlaying) diffVideoEl.pause();
    else diffVideoEl.play().catch(() => {});
  }

  function _seekDiffAt(clientX) {
    if (!diffTrackEl || !diffVideoEl || !diffDuration) return;
    const r = diffTrackEl.getBoundingClientRect();
    const f = Math.max(0, Math.min(1, (clientX - r.left) / r.width));
    diffVideoEl.currentTime = f * diffDuration;
  }

  function onDiffTrackDown(e) {
    if (!diffVideoEl || !diffDuration) return;
    diffDragging = true;
    diffVideoEl.pause();
    _seekDiffAt(e.clientX);
  }

  function onDiffWindowMouseMove(e) {
    if (diffDragging) _seekDiffAt(e.clientX);
  }

  function onDiffWindowMouseUp() {
    diffDragging = false;
    if (_qualityDragging) { _qualityDragging = false; onQualityEnd(); }
  }

  // ── Image quality diff ────────────────────────────────────────────────────
  let imgDiffMode       = $state(false);
  let imgDiffPath       = $state(null);
  let imgCompressedPath = $state(null);
  let imgDiffLoading    = $state(false);
  let _imgDiffTimer     = null;
  let _qualityDragging  = false;

  function _clearImageDiff() {
    imgDiffMode = false; imgDiffPath = null; imgCompressedPath = null;
  }

  async function _runImageDiff() {
    if (!selectedItem || selectedItem.mediaType !== 'image') return;
    imgDiffLoading = true;
    try {
      const result = await invoke('preview_image_quality', {
        path: selectedItem.path,
        quality: imageOptions.quality,
        outputFormat: imageOptions.output_format,
      });
      imgDiffPath       = result.diff_path;
      imgCompressedPath = result.compressed_path;
    } catch { /* non-fatal — lossless format or magick missing */ }
    finally { imgDiffLoading = false; }
  }

  function onQualityStart() {
    if (!['jpeg', 'webp'].includes(imageOptions.output_format)) return;
    _qualityDragging = true;
    imgDiffMode = true;
    _runImageDiff();
  }

  function onQualityInput() {
    if (!imgDiffMode) return;
    if (_imgDiffTimer) clearTimeout(_imgDiffTimer);
    _imgDiffTimer = setTimeout(_runImageDiff, 150);
  }

  function onQualityEnd() {
    if (_imgDiffTimer) { clearTimeout(_imgDiffTimer); _imgDiffTimer = null; }
    imgDiffMode = false;
    // Leave imgCompressedPath set so the preview shows the compressed result.
    // It clears on file change or format change.
  }

  // Clear image diff when the selected file or output format changes
  $effect(() => { selectedId; _clearImageDiff(); });
  $effect(() => { imageOptions.output_format; _clearImageDiff(); });

  // ── Crop state ─────────────────────────────────────────────────────────────
  let previewAreaEl  = $state(null);
  let imgEl          = $state(null);
  let imgNaturalW    = $state(0);
  let imgNaturalH    = $state(0);
  let cropActive     = $state(false);
  let cropAspect     = $state(null);
  let cropRect       = $state({ x: 0.1, y: 0.1, w: 0.8, h: 0.8 });
  let cropDrag       = $state(null);

  $effect(() => { selectedId; cropActive = false; imgNaturalW = 0; imgNaturalH = 0; });

  function onImgLoad(e) {
    imgNaturalW = e.currentTarget.naturalWidth;
    imgNaturalH = e.currentTarget.naturalHeight;
  }

  function getImgBounds() {
    if (!imgEl || !previewAreaEl) return null;
    const ir = imgEl.getBoundingClientRect();
    const pr = previewAreaEl.getBoundingClientRect();
    return { x: ir.left - pr.left, y: ir.top - pr.top, w: ir.width, h: ir.height };
  }

  function initCropRect(aspect) {
    if (!aspect || !imgNaturalW || !imgNaturalH) return { x: 0.1, y: 0.1, w: 0.8, h: 0.8 };
    const nr = aspect * imgNaturalH / imgNaturalW;
    if (nr <= 1) {
      const rw = 0.8, rh = rw / nr;
      if (rh <= 1) return { x: 0.1, y: (1 - rh) / 2, w: rw, h: rh };
      const rh2 = 0.8, rw2 = rh2 * nr;
      return { x: (1 - rw2) / 2, y: 0.1, w: rw2, h: rh2 };
    }
    const rh = 0.8, rw = rh * nr;
    if (rw <= 1) return { x: (1 - rw) / 2, y: 0.1, w: rw, h: rh };
    const rw2 = 0.8, rh2 = rw2 / nr;
    return { x: 0.1, y: (1 - rh2) / 2, w: rw2, h: rh2 };
  }

  function activateCrop(aspect) {
    cropAspect = aspect;
    cropRect = initCropRect(aspect);
    cropActive = true;
  }

  const CROP_MIN = 0.04;

  function startCropDrag(e, type) {
    e.stopPropagation();
    const b = getImgBounds();
    if (!b) return;
    cropDrag = { type, sx: e.clientX, sy: e.clientY, r0: { ...cropRect }, b };
  }

  function onCropDragMove(e) {
    if (!cropDrag) return;
    const { type, sx, sy, r0, b } = cropDrag;
    const dx = (e.clientX - sx) / b.w;
    const dy = (e.clientY - sy) / b.h;
    let { x, y, w, h } = r0;

    if (type === 'move') {
      x = Math.max(0, Math.min(1 - w, x + dx));
      y = Math.max(0, Math.min(1 - h, y + dy));
    } else {
      if (type.includes('w')) { const nx = Math.max(0, Math.min(x + w - CROP_MIN, x + dx)); w = x + w - nx; x = nx; }
      if (type.includes('e')) w = Math.max(CROP_MIN, Math.min(1 - x, w + dx));
      if (type.includes('n')) { const ny = Math.max(0, Math.min(y + h - CROP_MIN, y + dy)); h = y + h - ny; y = ny; }
      if (type.includes('s')) h = Math.max(CROP_MIN, Math.min(1 - y, h + dy));
      if (cropAspect && imgNaturalW && imgNaturalH) {
        const nr = cropAspect * imgNaturalH / imgNaturalW;
        if (type.includes('n') || type === 's') { w = Math.min(1 - x, Math.max(CROP_MIN, h * nr)); }
        else { h = Math.min(1 - y, Math.max(CROP_MIN, w / nr)); }
      }
    }
    cropRect = { x, y, w, h };
  }

  function onCropDragEnd() { cropDrag = null; }

  function applyCrop() {
    if (imgNaturalW && imgNaturalH) {
      imageOptions.crop_x = Math.round(cropRect.x * imgNaturalW);
      imageOptions.crop_y = Math.round(cropRect.y * imgNaturalH);
      imageOptions.crop_width = Math.round(cropRect.w * imgNaturalW);
      imageOptions.crop_height = Math.round(cropRect.h * imgNaturalH);
    }
    cropActive = false;
  }

  function cancelCrop() { cropActive = false; }

  function clearCropValues() {
    imageOptions.crop_x = 0; imageOptions.crop_y = 0;
    imageOptions.crop_width = null; imageOptions.crop_height = null;
    cropActive = false;
  }

  // Clear any diff preview when the selected file changes
  $effect(() => {
    selectedId;
    diffClipPath = null; diffError = null; diffNote = null;
  });

  async function runDiffPreview() {
    if (!selectedItem || selectedItem.mediaType !== 'video') return;
    const at = videoEl?.currentTime ?? 0;
    try { videoEl?.pause(); } catch { /* non-fatal */ }
    diffLoading = true;
    diffError = null;
    diffClipPath = null;
    diffNote = null;
    try {
      const result = await invoke('preview_diff', {
        path: selectedItem.path,
        codec: videoOptions.codec ?? 'h264',
        resolution: videoOptions.resolution ?? 'original',
        atSecs: at,
        durationSecs: 1.0,
        handleSecs: diffHandleSecs,
        amplify: 8.0,
      });
      diffClipPath = result.path;
      diffNote = result.note;
    } catch (e) {
      diffError = String(e);
    } finally {
      diffLoading = false;
    }
  }

  function dismissDiff() {
    diffClipPath = null; diffError = null; diffNote = null;
  }

  let imageOptions = $state({
    output_format: 'jpeg',
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
    for (const path of paths) {
      const name = path.split('/').pop() ?? path;
      const ext = name.includes('.') ? name.split('.').pop().toLowerCase() : '';
      const mt = mediaTypeFor(ext);
      const id = crypto.randomUUID();
      const item = { id, path, name, ext, mediaType: mt, status: 'pending', percent: 0, info: null };
      queue.push(item);
      // Fetch file stats in the background for display in the queue
      if (['video', 'audio', 'image'].includes(mt)) {
        invoke('get_file_info', { path }).then(info => {
          const q = queue.find(q => q.id === id);
          if (q) q.info = info;
        }).catch(() => {});
      }
    }
  }

  function removeItem(id) {
    queue = queue.filter(q => q.id !== id);
    if (selectedId === id) handleSelect(queue.length > 0 ? queue[0].id : null);
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

  async function startConvert(mode = 'all') {
    const errors = validateOptions(videoOptions, audioOptions);
    if (Object.keys(errors).length > 0) { validationErrors = errors; return; }
    validationErrors = {};

    if (!globalOutputFormat) {
      statusMessage = 'Select an output format first';
      return;
    }

    const candidates = mode === 'selected'
      ? (selectedItem ? [selectedItem] : [])
      : queue;

    const allPending = candidates.filter(q => q.status === 'pending' || q.status === 'error');
    const compat = compatibleTypes;
    const pending = allPending.filter(q => compat.includes(q.mediaType));
    const skipped = allPending.length - pending.length;
    if (pending.length === 0) {
      statusMessage = skipped > 0
        ? `No compatible files — ${skipped} skipped (incompatible)`
        : 'No files to convert';
      return;
    }

    converting = true;
    paused = false;
    statusMessage = 'Converting…';
    if (skipped > 0) statusMessage = `Converting… — ${skipped} skipped (incompatible)`;

    for (const item of pending) {
      if (paused) break;
      item.status = 'converting';
      item.percent = 0;

      const opts = item.mediaType === 'image'    ? { ...imageOptions,    output_suffix: outputSuffix, output_dir: outputDir }
             : item.mediaType === 'video'    ? { ...videoOptions,    output_suffix: outputSuffix, output_dir: outputDir }
             : item.mediaType === 'audio'    ? { ...audioOptions,    output_suffix: outputSuffix, output_dir: outputDir }
             : item.mediaType === 'data'     ? { ...dataOptions,     output_suffix: outputSuffix, output_dir: outputDir }
             : item.mediaType === 'document' ? { ...documentOptions, output_suffix: outputSuffix, output_dir: outputDir }
             :                                 { ...archiveOptions,   output_suffix: outputSuffix, output_dir: outputDir };

      invoke('convert_file', { jobId: item.id, inputPath: item.path, options: opts })
        .catch(err => { item.status = 'error'; item.error = String(err); checkAllDone(); });
    }
  }

  // ── Drag over window ───────────────────────────────────────────────────────

  let outputSuffix = $state('converted');
  let outputPickerOpen = $state(false);
  let outputDestMode = $state('source'); // 'source' | 'custom'
  let customOutputDir = $state(null);
  let folderInput = $state(null);
  let outputDir = $derived(outputDestMode === 'source' ? null : (customOutputDir ?? null));
  let dragOver = $state(false);

  function onFolderInputChange(e) {
    const files = Array.from(e.target.files ?? []);
    if (files.length > 0) {
      const p = files[0].path ?? files[0].webkitRelativePath ?? '';
      const dir = p.includes('/') ? p.substring(0, p.lastIndexOf('/')) : null;
      if (dir) { customOutputDir = dir; outputDestMode = 'custom'; }
    }
    e.target.value = '';
  }

  function onWindowDragover(e) { e.preventDefault(); dragOver = true; }
  function onWindowDragleave(e) { if (!e.relatedTarget) dragOver = false; }
  function onWindowDrop(e) {
    e.preventDefault();
    dragOver = false;
    const paths = Array.from(e.dataTransfer?.files ?? []).map(f => f.path ?? f.name);
    if (paths.length) addFiles(paths);
  }

  // ── Presets ────────────────────────────────────────────────────────────────

  let presets          = $state([]);
  let headerPresetId   = $state('');
  let headerAdding     = $state(false);
  let headerPresetName = $state('');
  let _hpSuppressReset = false; // plain bool, prevents auto-reset during apply

  // Auto-reset to "Presets" placeholder when the active settings change
  $effect(() => {
    void [
      imageOptions.output_format, imageOptions.quality,
      videoOptions.output_format, videoOptions.codec, videoOptions.bitrate, videoOptions.sample_rate,
      audioOptions.output_format, audioOptions.bitrate, audioOptions.sample_rate,
      activeOutputCategory,
    ];
    if (!_hpSuppressReset) headerPresetId = '';
  });

  // Sync globalOutputFormat into the relevant options object
  $effect(() => {
    if (!globalOutputFormat) return;
    const cat = categoryFor(globalOutputFormat);
    if (cat === 'audio')         audioOptions.output_format    = globalOutputFormat;
    else if (cat === 'video')    videoOptions.output_format    = globalOutputFormat;
    else if (cat === 'image')    imageOptions.output_format    = globalOutputFormat;
    else if (cat === 'data')     dataOptions.output_format     = globalOutputFormat;
    else if (cat === 'document') documentOptions.output_format = globalOutputFormat;
    else if (cat === 'archive')  archiveOptions.output_format  = globalOutputFormat;
  });

  async function loadPresets() {
    try { presets = await invoke('list_presets'); } catch { /* no-op */ }
  }

  function applyPreset(id) {
    const p = presets.find(p => p.id === id);
    if (!p) return;
    _hpSuppressReset = true;
    if (p.media_type === 'image') {
      imageOptions.output_format = p.output_format;
      if (p.quality != null) imageOptions.quality = p.quality;
    } else if (p.media_type === 'video') {
      videoOptions.output_format = p.output_format;
      if (p.codec != null) videoOptions.codec = p.codec;
      if (p.bitrate != null) videoOptions.bitrate = p.bitrate;
      if (p.sample_rate != null) videoOptions.sample_rate = p.sample_rate;
    } else {
      audioOptions.output_format = p.output_format;
      if (p.bitrate != null) audioOptions.bitrate = p.bitrate;
      if (p.sample_rate != null) audioOptions.sample_rate = p.sample_rate;
    }
    queueMicrotask(() => { _hpSuppressReset = false; });
  }

  async function saveHeaderPreset() {
    const name = headerPresetName.trim();
    if (!name || !activeOutputCategory) return;
    const tab = activeOutputCategory;
    const src = tab === 'image' ? imageOptions : tab === 'video' ? videoOptions : audioOptions;
    try {
      const saved = await invoke('save_preset', {
        name, mediaType: tab,
        outputFormat: src.output_format,
        quality: tab === 'image' ? (src.quality ?? null) : null,
        codec: tab === 'video' ? (src.codec ?? null) : null,
        bitrate: (tab === 'video' || tab === 'audio') ? (src.bitrate ?? null) : null,
        sampleRate: (tab === 'video' || tab === 'audio') ? (src.sample_rate ?? null) : null,
      });
      presets = [...presets, saved];
      headerPresetName = '';
      headerAdding = false;
      _hpSuppressReset = true;
      headerPresetId = saved.id;
      queueMicrotask(() => { _hpSuppressReset = false; });
    } catch (e) { console.error('Save preset failed:', e); }
  }

  async function deletePreset(id) {
    try {
      await invoke('delete_preset', { id });
      presets = presets.filter(p => p.id !== id);
      if (headerPresetId === id) headerPresetId = '';
    } catch (e) { console.error('Delete preset failed:', e); }
  }

  // ── Tooltip bar ────────────────────────────────────────────────────────────
  let tooltipText = $state('');
  function onPanelMouseOver(e) {
    const el = e.target.closest('[data-tooltip]');
    tooltipText = el?.dataset.tooltip ?? '';
  }

  // ── Output format state ────────────────────────────────────────────────────
  let globalOutputFormat = $state(null); // null = nothing selected

  const FORMAT_GROUPS = [
    { label: 'Audio', cat: 'audio', fmts: [
      { id: 'mp3' }, { id: 'wav' }, { id: 'flac' }, { id: 'ogg' },
      { id: 'aac' }, { id: 'opus' }, { id: 'm4a' }, { id: 'wma' },
      { id: 'aiff' }, { id: 'alac' }, { id: 'ac3' }, { id: 'dts' },
      { id: 'vorbis', label: 'Vorbis', todo: true },
      { id: 'ddp', label: 'Dolby Digital+', todo: true },
      { id: 'truehd', label: 'Dolby TrueHD', todo: true },
    ]},
    { label: 'Video', cat: 'video', fmts: [
      { id: 'mp4' }, { id: 'mov' }, { id: 'webm' }, { id: 'mkv' }, { id: 'avi' }, { id: 'gif' },
      { id: 'm4v',   todo: true }, { id: 'flv',   todo: true }, { id: 'mpg',  todo: true },
      { id: 'ogv',   todo: true }, { id: 'ts',    todo: true }, { id: '3gp',  todo: true },
      { id: 'divx',  todo: true }, { id: 'rmvb',  todo: true }, { id: 'asf',  todo: true },
      { id: 'prores', label: 'Apple ProRes', todo: true },
      { id: 'dnxhd', label: 'DNxHD', todo: true },
      { id: 'dnxhr', label: 'DNxHR', todo: true },
      { id: 'cineform', label: 'CineForm', todo: true },
      { id: 'qt-anim', label: 'QT Animation', todo: true },
      { id: 'uncompressed', label: 'Uncompressed', todo: true },
      { id: 'ffv1', label: 'FFV1', todo: true },
      { id: 'xdcam-422', label: 'XDCAM HD422', todo: true },
      { id: 'xdcam-35', label: 'XDCAM HD35', todo: true },
      { id: 'avc-intra', label: 'AVC-Intra', todo: true },
      { id: 'xavc', label: 'XAVC', todo: true },
      { id: 'xavc-lgop', label: 'XAVC Long GOP', todo: true },
      { id: 'hap', label: 'HAP', todo: true },
      { id: 'theora', label: 'Theora', todo: true },
      { id: 'mpeg2', label: 'MPEG-2', todo: true },
      { id: 'mjpeg', label: 'MJPEG', todo: true },
      { id: 'xvid', label: 'Xvid', todo: true },
      { id: 'dv', label: 'DV', todo: true },
      { id: 'wmv', label: 'WMV', todo: true },
      { id: 'mpeg1', label: 'MPEG-1', todo: true },
    ]},
    { label: 'Image', cat: 'image', fmts: [
      { id: 'jpeg' }, { id: 'png' }, { id: 'webp' }, { id: 'tiff' }, { id: 'bmp' }, { id: 'avif' },
      { id: 'gif',   todo: true }, { id: 'svg',  todo: true }, { id: 'ico',  todo: true },
      { id: 'jpegxl', label: 'JPEG XL', todo: true },
      { id: 'heic',  todo: true }, { id: 'heif', todo: true }, { id: 'psd',  todo: true },
      { id: 'exr',   todo: true }, { id: 'hdr',  todo: true }, { id: 'dds',  todo: true },
      { id: 'xcf',   todo: true },
      { id: 'raw',   todo: true }, { id: 'cr2',  todo: true }, { id: 'cr3',  todo: true },
      { id: 'nef',   todo: true }, { id: 'arw',  todo: true }, { id: 'dng',  todo: true },
      { id: 'orf',   todo: true }, { id: 'rw2',  todo: true },
    ]},
    { label: 'Data', cat: 'data', fmts: [
      { id: 'json' }, { id: 'csv' }, { id: 'tsv' }, { id: 'xml' }, { id: 'yaml' },
    ]},
    { label: 'Document', cat: 'document', fmts: [
      { id: 'html' }, { id: 'pdf' }, { id: 'txt' }, { id: 'md' },
    ]},
    { label: 'Archive', cat: 'archive', fmts: [
      { id: 'zip' }, { id: 'tar' }, { id: 'gz' }, { id: '7z' },
    ]},
    { label: '3D Model', cat: '3d', fmts: [
      { id: 'obj',  todo: true }, { id: 'gltf', todo: true }, { id: 'glb',  todo: true },
      { id: 'stl',  todo: true }, { id: 'fbx',  todo: true }, { id: 'ply',  todo: true },
      { id: '3ds',  todo: true },
    ]},
    { label: 'Operations', cat: 'ops', fmts: [
      { id: 'cut-noenc', label: 'Cut (no re-encode)', todo: true },
      { id: 'replace-audio', label: 'Replace Audio', todo: true },
      { id: 'rewrap', label: 'Rewrap', todo: true },
      { id: 'conform', label: 'Conform', todo: true },
      { id: 'merge', label: 'Merge', todo: true },
      { id: 'extract', label: 'Extract', todo: true },
      { id: 'subtitling', label: 'Subtitling', todo: true },
      { id: 'video-inserts', label: 'Video Inserts', todo: true },
    ]},
    { label: 'AI Tools', cat: 'ai', fmts: [
      { id: 'ai-sep', label: 'Audio Separation', todo: true },
      { id: 'ai-transcribe', label: 'Transcription', todo: true },
      { id: 'ai-translate', label: 'Translate', todo: true },
      { id: 'ai-colorize', label: 'Colorize', todo: true },
      { id: 'ai-bgremove', label: 'BG Remover', todo: true },
    ]},
    { label: 'Analysis', cat: 'analysis', fmts: [
      { id: 'loudness', label: 'Loudness & TP', todo: true },
      { id: 'audio-norm', label: 'Audio Norm', todo: true },
      { id: 'cut-detect', label: 'Cut Detection', todo: true },
      { id: 'black-detect', label: 'Black Detection', todo: true },
      { id: 'vmaf', label: 'VMAF', todo: true },
      { id: 'framemd5', label: 'FrameMD5', todo: true },
    ]},
    { label: 'Burn & Rip', cat: 'burn', fmts: [
      { id: 'dvd', label: 'DVD', todo: true },
      { id: 'bluray', label: 'Blu-ray', todo: true },
      { id: 'dvd-rip', label: 'DVD Rip', todo: true },
      { id: 'web-video', label: 'Web Video', todo: true },
    ]},
  ];

  function categoryFor(fmt) {
    if (!fmt) return null;
    for (const g of FORMAT_GROUPS) {
      if (g.fmts.some(f => f.id === fmt)) return g.cat;
    }
    return null;
  }

  // Which output categories are reachable from the selected input's media type.
  // null = no file selected = no filter.
  const OUTPUT_CATS_FOR = {
    video:    ['video', 'audio'],
    audio:    ['audio'],
    image:    ['image'],
    data:     ['data'],
    document: ['document'],
    archive:  ['archive'],
  };
  let compatibleOutputCats = $derived(
    selectedItem ? (OUTPUT_CATS_FOR[selectedItem.mediaType] ?? null) : null
  );

  let activeOutputCategory = $derived(categoryFor(globalOutputFormat));

  let compatibleTypes = $derived(
    !activeOutputCategory ? [] :
    activeOutputCategory === 'audio' ? ['audio', 'video'] :
    activeOutputCategory === 'video' ? ['video'] :
    activeOutputCategory === 'image' ? ['image'] :
    activeOutputCategory === 'data'  ? ['data'] :
    activeOutputCategory === 'document' ? ['document'] :
    activeOutputCategory === 'archive'  ? ['archive'] :
    []
  );

  // ── Selection handler ──────────────────────────────────────────────────────
  // All mutations happen synchronously in the same call so Svelte batches them
  // into ONE DOM flush — guaranteed clear before any file I/O begins.
  // Then the async pipeline runs each loading stage in sequence.
  function handleSelect(id) {
    const gen = ++_generation;  // cancel any in-flight pipeline
    const newItem = id ? queue.find(q => q.id === id) : null;
    const isMedia = !!(newItem && ['video', 'audio', 'image'].includes(newItem.mediaType));

    // ── Synchronous batch: clears everything in one Svelte flush ──
    liveSrc            = null;
    selectedDuration   = null;
    tlMediaReady       = false;
    tlWaveformReady    = false;
    tlSpectrogramReady = false;
    tlFilmstripReady   = false;
    previewLoading     = isMedia;
    videoEl?.load();   // flash existing video to black immediately
    selectedId         = id ?? null;

    // ── Async pipeline: stages run sequentially after browser paints ──
    runLoadPipeline(gen, newItem);
  }

</script>

<svelte:window
  onmousemove={(e) => { onDiffWindowMouseMove(e); onCropDragMove(e); }}
  onmouseup={() => { onDiffWindowMouseUp(); onCropDragEnd(); }}
/>

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
        <!-- List view toggle -->
        <div class="flex items-center gap-0.5 ml-auto">
          <button
            onclick={() => queueCompact = false}
            title="Expanded list"
            class="w-6 h-6 flex items-center justify-center rounded transition-colors
                   {!queueCompact ? 'text-[var(--text-primary)] bg-white/10' : 'text-[var(--text-secondary)] hover:text-[var(--text-primary)]'}"
          >
            <svg width="12" height="13" viewBox="0 0 12 13" fill="currentColor">
              <rect y="0"    width="12" height="2"   rx="0.5"/>
              <rect y="3.5"  width="12" height="2"   rx="0.5"/>
              <rect y="7"    width="12" height="2"   rx="0.5"/>
              <rect y="10.5" width="12" height="2"   rx="0.5"/>
            </svg>
          </button>
          <button
            onclick={() => queueCompact = true}
            title="Compact list"
            class="w-6 h-6 flex items-center justify-center rounded transition-colors
                   {queueCompact ? 'text-[var(--text-primary)] bg-white/10' : 'text-[var(--text-secondary)] hover:text-[var(--text-primary)]'}"
          >
            <svg width="12" height="9" viewBox="0 0 12 9" fill="currentColor">
              <rect y="0" width="12" height="3" rx="0.5"/>
              <rect y="6" width="12" height="3" rx="0.5"/>
            </svg>
          </button>
        </div>
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
        onselect={handleSelect}
        onremove={(id) => removeItem(id)}
        oncancel={(id) => cancelJob(id)}
        oninfo={(item) => showFileInfo(item)}
        compatibleTypes={compatibleTypes}
        compact={queueCompact}
      />

      <!-- Hidden folder picker input -->
      <input
        type="file"
        bind:this={folderInput}
        onchange={onFolderInputChange}
        class="hidden"
        aria-hidden="true"
        webkitdirectory
      />

      <!-- ── Bottom panel: output + convert + settings ──────────────────────── -->
      <div class="shrink-0 border-t border-[var(--border)] flex flex-col gap-2 px-3 py-2.5"
           style="background:color-mix(in srgb, var(--surface-raised) 60%, #000 40%)">

        <!-- Overall progress -->
        <ProgressBar value={overallPercent} />

        <!-- Output destination -->
        <div class="flex flex-col gap-1">
          <!-- svelte-ignore a11y_no_static_element_interactions -->
          <div class="flex gap-1.5">
            <button
              onclick={() => outputDestMode = 'source'}
              class="flex items-center gap-1.5 px-2 py-1 rounded text-[11px] border transition-colors flex-1
                     {outputDestMode === 'source'
                       ? 'border-[var(--accent)] text-[var(--accent)] bg-[var(--accent)]/10'
                       : 'border-[var(--border)] text-[var(--text-secondary)] hover:border-[var(--accent)] hover:text-[var(--accent)]'}"
            >
              <span class="w-2 h-2 rounded-full border shrink-0 flex items-center justify-center
                           {outputDestMode === 'source' ? 'border-[var(--accent)]' : 'border-[var(--text-secondary)]'}">
                {#if outputDestMode === 'source'}<span class="w-1 h-1 rounded-full bg-[var(--accent)]"></span>{/if}
              </span>
              Source folder
            </button>
            <button
              onclick={() => { outputDestMode = 'custom'; if (!customOutputDir) folderInput?.click(); }}
              class="flex items-center gap-1.5 px-2 py-1 rounded text-[11px] border transition-colors flex-1
                     {outputDestMode === 'custom'
                       ? 'border-[var(--accent)] text-[var(--accent)] bg-[var(--accent)]/10'
                       : 'border-[var(--border)] text-[var(--text-secondary)] hover:border-[var(--accent)] hover:text-[var(--accent)]'}"
            >
              <span class="w-2 h-2 rounded-full border shrink-0 flex items-center justify-center
                           {outputDestMode === 'custom' ? 'border-[var(--accent)]' : 'border-[var(--text-secondary)]'}">
                {#if outputDestMode === 'custom'}<span class="w-1 h-1 rounded-full bg-[var(--accent)]"></span>{/if}
              </span>
              Custom…
            </button>
          </div>
          {#if outputDestMode === 'custom'}
            <div class="flex gap-1">
              <span class="flex-1 min-w-0 px-2 py-0.5 rounded text-[11px] font-mono border border-[var(--border)]
                           bg-[var(--surface)] text-[var(--text-secondary)] truncate">
                {customOutputDir ?? '—'}
              </span>
              <button
                onclick={() => folderInput?.click()}
                class="px-2 py-0.5 rounded text-[11px] border border-[var(--border)]
                       text-[var(--text-secondary)] hover:text-[var(--accent)] hover:border-[var(--accent)] transition-colors shrink-0"
              >Browse</button>
            </div>
          {/if}
        </div>

        <!-- Suffix -->
        <div class="flex items-center gap-2">
          <label for="output-suffix" class="text-[11px] text-[var(--text-secondary)] whitespace-nowrap shrink-0">Suffix</label>
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
        </div>

        <!-- Status message -->
        {#if statusMessage}
          <p class="text-[11px] text-[var(--text-secondary)] truncate" aria-live="polite">{statusMessage}</p>
        {/if}

        <!-- Convert / Pause / Cancel -->
        {#if converting}
          <div class="flex gap-1.5">
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
          </div>
        {:else}
          <div class="flex gap-1.5">
            <button
              onclick={() => startConvert('selected')}
              disabled={!selectedItem || queue.length === 0 || !globalOutputFormat}
              class="flex-1 py-1.5 rounded text-[12px] font-medium transition-colors border
                     {!selectedItem || queue.length === 0 || !globalOutputFormat
                       ? 'border-[var(--border)] text-[var(--text-secondary)] cursor-not-allowed opacity-40'
                       : 'border-[var(--border)] text-[var(--text-primary)] hover:border-[var(--accent)] hover:text-[var(--accent)]'}"
            >Convert Selected</button>
            <button
              onclick={() => startConvert('all')}
              disabled={queue.length === 0 || !globalOutputFormat}
              class="flex-1 py-1.5 rounded text-[12px] font-semibold transition-colors
                     {queue.length === 0 || !globalOutputFormat
                       ? 'bg-[var(--border)] text-[var(--text-secondary)] cursor-not-allowed opacity-40'
                       : 'bg-[var(--accent)] text-white hover:opacity-90'}"
            >Convert All</button>
          </div>
        {/if}

        <!-- Settings button -->
        <button
          class="flex items-center gap-2 px-2.5 py-1.5 rounded text-[12px] border border-[var(--border)]
                 text-[var(--text-secondary)] hover:text-[var(--text-primary)] hover:border-[var(--accent)]
                 transition-colors w-full"
        >
          <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor"
               stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round" class="shrink-0">
            <circle cx="12" cy="12" r="3"/>
            <path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 0 1-2.83 2.83l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-4 0v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 0 1-2.83-2.83l.06-.06A1.65 1.65 0 0 0 4.68 15a1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1 0-4h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 0 1 2.83-2.83l.06.06A1.65 1.65 0 0 0 9 4.68a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 4 0v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 0 1 2.83 2.83l-.06.06A1.65 1.65 0 0 0 19.4 9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 0 4h-.09a1.65 1.65 0 0 0-1.51 1z"/>
          </svg>
          Settings
        </button>
      </div>

    </aside>

    <!-- ── CENTER: Preview + timeline ─────────────────────────────────────── -->
    <div class="flex flex-col flex-1 min-w-0">

      <!-- Preview area -->
      <div class="flex-1 min-h-0 bg-[#1a1a1a] flex items-center justify-center relative overflow-hidden" bind:this={previewAreaEl}>
        <!-- ── VIDEO: lives outside {#key} so videoEl is NEVER null while a
               video is selected — prevents Timeline falling back to new Audio() ── -->
        {#if selectedItem?.mediaType === 'video'}
          <!-- svelte-ignore a11y_media_has_caption -->
          <video
            bind:this={videoEl}
            src={liveSrc ?? undefined}
            preload="metadata"
            onloadedmetadata={onPreviewLoaded}
            class="max-w-full max-h-full object-contain {(!liveSrc || diffClipPath) ? 'hidden' : ''}"
          ></video>
            {#if diffClipPath}
              <!-- svelte-ignore a11y_media_has_caption -->
              <video
                bind:this={diffVideoEl}
                src={convertFileSrc(diffClipPath)}
                autoplay
                loop
                muted
                onplay={() => diffPlaying = true}
                onpause={() => diffPlaying = false}
                ontimeupdate={(e) => diffCurrentTime = e.currentTarget.currentTime}
                onloadedmetadata={(e) => diffDuration = e.currentTarget.duration}
                class="max-w-full max-h-full object-contain"
              ></video>

              <!-- Floating mini scrubber — bottom-centre -->
              <div class="absolute left-1/2 bottom-4 -translate-x-1/2 z-20
                          flex items-center gap-2 px-2.5 py-1.5
                          rounded-full bg-black/70 backdrop-blur-sm
                          border border-white/10 shadow-lg"
                   style="width:min(60%, 420px)">
                <button
                  onclick={toggleDiffPlay}
                  class="w-6 h-6 shrink-0 flex items-center justify-center rounded-full
                         bg-white/10 hover:bg-white/20 transition-colors"
                  aria-label={diffPlaying ? 'Pause' : 'Play'}
                >
                  {#if diffPlaying}
                    <svg width="10" height="10" viewBox="0 0 24 24" fill="white">
                      <rect x="6" y="4" width="4" height="16"/><rect x="14" y="4" width="4" height="16"/>
                    </svg>
                  {:else}
                    <svg width="10" height="10" viewBox="0 0 24 24" fill="white">
                      <path d="M8 5v14l11-7z"/>
                    </svg>
                  {/if}
                </button>
                <!-- svelte-ignore a11y_click_events_have_key_events -->
                <!-- svelte-ignore a11y_no_static_element_interactions -->
                <div
                  bind:this={diffTrackEl}
                  onmousedown={onDiffTrackDown}
                  class="flex-1 h-1 rounded-full bg-white/15 relative cursor-pointer"
                >
                  <div class="absolute inset-y-0 left-0 rounded-full bg-white/80"
                       style="width:{diffDuration ? (diffCurrentTime / diffDuration) * 100 : 0}%"></div>
                  <div class="absolute top-1/2 -translate-y-1/2 -translate-x-1/2 w-2.5 h-2.5 rounded-full bg-white shadow"
                       style="left:{diffDuration ? (diffCurrentTime / diffDuration) * 100 : 0}%"></div>
                </div>
                <span class="shrink-0 font-mono tabular-nums text-[10px] text-white/70">
                  {diffDuration ? `${diffCurrentTime.toFixed(2)}s` : '—'}
                </span>
              </div>
            {/if}

            <!-- Diff controls overlay (top-right) -->
            <div class="absolute top-2 right-2 z-10 flex items-center gap-1.5">
              {#if diffNote}
                <span class="px-2 py-0.5 rounded bg-black/60 text-white/70 font-mono text-[10px]">
                  {diffNote}
                </span>
              {/if}
              {#if diffClipPath}
                <button
                  onclick={dismissDiff}
                  class="px-2 py-1 rounded bg-[var(--accent)] text-white text-[11px] font-medium hover:opacity-90"
                  title="Return to source preview"
                >Exit diff</button>
              {:else}
                <select
                  bind:value={diffHandleSecs}
                  disabled={diffLoading}
                  title="Runway (handle) length on each side of the target region.
1s — fast, less accurate rate control
3s — accurate for CRF x264 / x265
10s — needed for long-GOP codecs (AV1)"
                  class="px-1.5 py-1 rounded bg-black/60 border border-white/15 text-white text-[11px] font-mono
                         hover:border-[var(--accent)] transition-colors disabled:opacity-50 outline-none"
                >
                  <option value={1}>1s · fast</option>
                  <option value={3}>3s · accurate</option>
                  <option value={10}>10s · AV1-safe</option>
                </select>
                <button
                  onclick={runDiffPreview}
                  disabled={diffLoading}
                  class="px-2 py-1 rounded bg-black/60 border border-white/15 text-white text-[11px] font-medium
                         hover:bg-black/80 hover:border-[var(--accent)] transition-colors disabled:opacity-50"
                  title="Encode a 1s snippet at the current cursor with handles on each side and show the amplified difference vs. the source"
                >
                  {#if diffLoading}Encoding…{:else}Diff preview{/if}
                </button>
              {/if}
            </div>

            {#if diffError}
              <div class="absolute bottom-2 left-2 right-2 z-10 px-3 py-2 rounded
                          bg-red-950/80 border border-red-800 text-red-200 text-[11px] font-mono">
                {diffError}
              </div>
            {/if}
        {/if}

        <!-- ── NON-VIDEO content: key block remounts on each selection ── -->
        {#key selectedId}
          {#if selectedItem?.mediaType === 'image' && liveSrc}
            {#if imgDiffMode && imgDiffPath && !cropActive}
              <img src={convertFileSrc(imgDiffPath)} alt="Quality diff"
                   class="max-w-full max-h-full object-contain" />
            {:else if !imgDiffMode && imgCompressedPath && !cropActive}
              <img bind:this={imgEl} src={convertFileSrc(imgCompressedPath)} alt="Compressed preview"
                   class="max-w-full max-h-full object-contain"
                   onload={(e) => { onImgLoad(e); onPreviewLoaded(); }} />
            {:else}
              <img bind:this={imgEl} src={liveSrc} alt={selectedItem.name}
                   class="max-w-full max-h-full object-contain"
                   onload={(e) => { onImgLoad(e); onPreviewLoaded(); }} />
            {/if}

            <!-- Quality mode badge -->
            {#if imgDiffMode || imgCompressedPath}
              <div class="absolute top-2 left-1/2 -translate-x-1/2 z-10
                          px-2.5 py-0.5 rounded-full bg-black/65 border border-white/10
                          text-white/80 text-[10px] font-mono select-none pointer-events-none">
                {#if imgDiffLoading}updating…
                {:else if imgDiffMode}diff · Q{imageOptions.quality}
                {:else}compressed · Q{imageOptions.quality}
                {/if}
              </div>
            {/if}

            <!-- Crop overlay -->
            {#if cropActive && imgEl}
              {@const ib = getImgBounds()}
              {#if ib}
                {@const cx = ib.x + cropRect.x * ib.w}
                {@const cy = ib.y + cropRect.y * ib.h}
                {@const cw = cropRect.w * ib.w}
                {@const ch = cropRect.h * ib.h}
                <!-- svelte-ignore a11y_no_static_element_interactions -->
                <div class="absolute inset-0 z-30 select-none" style="cursor:default">
                  <!-- Dim: top -->
                  <div class="absolute bg-black/50 pointer-events-none"
                       style="left:{ib.x}px; top:{ib.y}px; width:{ib.w}px; height:{cropRect.y * ib.h}px"></div>
                  <!-- Dim: bottom -->
                  <div class="absolute bg-black/50 pointer-events-none"
                       style="left:{ib.x}px; top:{cy + ch}px; width:{ib.w}px; height:{ib.h - (cropRect.y + cropRect.h) * ib.h}px"></div>
                  <!-- Dim: left -->
                  <div class="absolute bg-black/50 pointer-events-none"
                       style="left:{ib.x}px; top:{cy}px; width:{cropRect.x * ib.w}px; height:{ch}px"></div>
                  <!-- Dim: right -->
                  <div class="absolute bg-black/50 pointer-events-none"
                       style="left:{cx + cw}px; top:{cy}px; width:{ib.w - (cropRect.x + cropRect.w) * ib.w}px; height:{ch}px"></div>

                  <!-- Crop border -->
                  <div class="absolute pointer-events-none"
                       style="left:{cx}px; top:{cy}px; width:{cw}px; height:{ch}px; border:1.5px solid rgba(255,255,255,0.85); box-sizing:border-box">
                    <!-- Rule-of-thirds grid -->
                    <div class="absolute inset-y-0" style="left:33.33%; border-left:1px solid rgba(255,255,255,0.3)"></div>
                    <div class="absolute inset-y-0" style="left:66.66%; border-left:1px solid rgba(255,255,255,0.3)"></div>
                    <div class="absolute inset-x-0" style="top:33.33%; border-top:1px solid rgba(255,255,255,0.3)"></div>
                    <div class="absolute inset-x-0" style="top:66.66%; border-top:1px solid rgba(255,255,255,0.3)"></div>
                  </div>

                  <!-- Move area -->
                  <!-- svelte-ignore a11y_no_static_element_interactions -->
                  <div class="absolute" style="left:{cx}px; top:{cy}px; width:{cw}px; height:{ch}px; cursor:move; z-index:1"
                       onmousedown={(e) => startCropDrag(e, 'move')}></div>

                  <!-- Corner handles -->
                  {#each [['nw',cx,cy,'nwse-resize'],['ne',cx+cw,cy,'nesw-resize'],['sw',cx,cy+ch,'nesw-resize'],['se',cx+cw,cy+ch,'nwse-resize']] as [type,hx,hy,cur]}
                    <!-- svelte-ignore a11y_no_static_element_interactions -->
                    <div class="absolute w-3 h-3 bg-white rounded-sm shadow"
                         style="left:{hx}px; top:{hy}px; transform:translate(-50%,-50%); cursor:{cur}; z-index:3"
                         onmousedown={(e) => startCropDrag(e, type)}></div>
                  {/each}

                  <!-- Edge handles -->
                  {#each [['n',cx+cw/2,cy,'ns-resize'],['s',cx+cw/2,cy+ch,'ns-resize'],['w',cx,cy+ch/2,'ew-resize'],['e',cx+cw,cy+ch/2,'ew-resize']] as [type,hx,hy,cur]}
                    <!-- svelte-ignore a11y_no_static_element_interactions -->
                    <div class="absolute w-3 h-3 bg-white rounded-sm shadow"
                         style="left:{hx}px; top:{hy}px; transform:translate(-50%,-50%); cursor:{cur}; z-index:3"
                         onmousedown={(e) => startCropDrag(e, type)}></div>
                  {/each}

                  <!-- Apply / Cancel — always visible inside the overlay -->
                  <div class="absolute bottom-3 right-3 flex gap-2" style="z-index:10">
                    <button onclick={cancelCrop}
                            class="px-3 py-1 rounded bg-black/60 border border-white/15 text-white text-[11px] hover:bg-black/80 transition-colors">
                      Cancel
                    </button>
                    <button onclick={applyCrop}
                            class="px-3 py-1 rounded bg-[var(--accent)] text-white text-[11px] font-medium hover:opacity-90 transition-opacity">
                      Apply
                    </button>
                  </div>
                </div>
              {/if}
            {/if}
          {:else if selectedItem && !['video','audio','image'].includes(selectedItem.mediaType)}
            <!-- Non-media types: show file info -->
            <div class="text-center select-none">
              <p class="text-white/20 text-[11px] font-mono uppercase tracking-widest mb-2">
                {selectedItem.ext}
              </p>
              <p class="text-white/40 text-[13px] truncate max-w-xs px-4">
                {selectedItem.name}
              </p>
            </div>
          {:else if !selectedItem}
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
          <!-- Loading text — shown while preview is clearing/loading -->
          {#if previewLoading}
            <p class="absolute select-none text-[40px] font-medium"
               style="color:rgba(255,255,255,0.25)">Loading</p>
          {/if}
        {/key}
      </div>

      <!-- Loading bar — sits between preview and timeline -->
      <div class="shrink-0 h-[2px] relative overflow-hidden" style="background:var(--border)">
        {#if previewLoading}
          <div class="preview-loading-bar absolute inset-y-0 w-1/3 rounded-full"
               style="background:var(--accent)"></div>
        {/if}
      </div>

      <!-- Timeline -->
      {#if selectedItem?.mediaType === 'video'}
        <Timeline item={selectedItem} duration={selectedDuration} bind:options={videoOptions} mediaEl={videoEl} onscrubstart={dismissDiff} bind:vizExpanded mediaReady={tlMediaReady} waveformReady={tlWaveformReady} spectrogramReady={tlSpectrogramReady} filmstripReady={tlFilmstripReady} />
      {:else if selectedItem?.mediaType === 'audio'}
        <Timeline item={selectedItem} duration={selectedDuration} bind:options={audioOptions} onscrubstart={dismissDiff} bind:vizExpanded mediaReady={tlMediaReady} waveformReady={tlWaveformReady} spectrogramReady={tlSpectrogramReady} />
      {:else}
        <div class="h-28 shrink-0 border-t border-[var(--border)] flex items-center justify-center"
             style="background:#0a0a0a">
          <span class="text-[11px]" style="color:#333">—</span>
        </div>
      {/if}

      <!-- Tooltip bar — crossfade: 50ms in, 150ms out, overlapping spans -->
      <div class="shrink-0 border-t border-[var(--border)] relative overflow-hidden"
           style="height:24px; background:color-mix(in srgb, var(--surface-raised) 60%, #000 40%)">
        {#key tooltipText}
          {#if tooltipText}
            <span class="absolute inset-0 flex items-center px-3 text-[11px] truncate"
                  style="color:rgba(255,255,255,0.5)"
                  in:fade={{ duration: 50 }}
                  out:fade={{ duration: 150 }}>
              {tooltipText}
            </span>
          {/if}
        {/key}
      </div>
    </div>

    <!-- ── RIGHT: Options panel (333px) ─────────────────────────────────────── -->
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <aside class="w-[333px] shrink-0 border-l border-[var(--border)] flex flex-col bg-[var(--surface-raised)] relative"
           role="region" aria-label="Conversion options"
           onmouseover={onPanelMouseOver}
           onfocus={onPanelMouseOver}
           onmouseleave={() => { tooltipText = ''; }}
           onblur={() => tooltipText = ''}>

      <!-- ── Panel header: Output picker + Presets ────────────────────────────── -->
      <div class="flex items-center gap-1 px-3 py-2 border-b border-[var(--border)] shrink-0 relative">

        <!-- Output format button — toggles picker in body -->
        <button
          onclick={() => { outputPickerOpen = !outputPickerOpen; }}
          class="px-2 py-1 rounded text-[12px] border transition-colors flex items-center gap-1.5
                 {globalOutputFormat && !outputPickerOpen
                   ? 'border-[var(--accent)] text-[var(--accent)]'
                   : 'border-[var(--border)] text-[var(--text-secondary)] hover:border-[var(--accent)] hover:text-[var(--accent)]'}"
        >
          {globalOutputFormat && !outputPickerOpen
            ? FORMAT_GROUPS.find(g => g.fmts.some(f => f.id === globalOutputFormat))?.fmts.find(f => f.id === globalOutputFormat)?.label?.toUpperCase() ?? globalOutputFormat.toUpperCase()
            : 'Output'}
          <svg width="8" height="5" viewBox="0 0 8 5" fill="none" stroke="currentColor"
               stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"
               class="shrink-0 transition-transform {outputPickerOpen || !globalOutputFormat ? 'rotate-180' : ''}">
            <path d="M1 1l3 3 3-3"/>
          </svg>
        </button>

        <!-- Presets selector -->
        {#if activeOutputCategory && ['image','video','audio'].includes(activeOutputCategory)}
          {#if headerAdding}
            <div class="flex gap-1 ml-auto">
              <!-- svelte-ignore a11y_autofocus -->
              <input
                type="text"
                bind:value={headerPresetName}
                placeholder="Name…"
                autofocus
                onkeydown={(e) => { if (e.key === 'Enter') saveHeaderPreset(); if (e.key === 'Escape') { headerAdding = false; headerPresetName = ''; } }}
                class="w-24 px-2 py-1 rounded-l text-[12px] border border-[var(--border)]
                       bg-[var(--surface)] text-[var(--text-primary)] outline-none
                       focus:border-[var(--accent)] transition-colors"
              />
              <button onclick={saveHeaderPreset}
                      class="px-2 py-1 text-[12px] border -ml-px border-[var(--accent)]
                             bg-[var(--accent)] text-white rounded-r hover:opacity-90 transition-opacity">
                Save
              </button>
              <button onclick={() => { headerAdding = false; headerPresetName = ''; }}
                      class="px-2 py-1 text-[12px] border border-[var(--border)] rounded
                             text-[var(--text-secondary)] hover:text-[var(--text-primary)] transition-colors">
                ✕
              </button>
            </div>
          {:else}
            <div class="flex gap-1 ml-auto">
              <select
                bind:value={headerPresetId}
                onchange={(e) => { const id = e.currentTarget.value; if (id) applyPreset(id); }}
                class="px-2 py-1 rounded text-[12px] border border-[var(--border)]
                       bg-[var(--surface)] outline-none transition-colors cursor-pointer
                       {headerPresetId ? 'text-[var(--text-primary)]' : 'text-[var(--text-secondary)]'}"
              >
                <option value="">Presets</option>
                {#each presets.filter(p => p.media_type === activeOutputCategory) as p (p.id)}
                  <option value={p.id}>{p.name}</option>
                {/each}
              </select>
              <button
                onclick={() => { headerAdding = true; headerPresetName = ''; }}
                title="Save current settings as preset"
                class="px-2 py-1 text-[12px] border border-[var(--border)] rounded
                       text-[var(--text-secondary)] hover:text-[var(--accent)] hover:border-[var(--accent)]
                       transition-colors leading-none"
              >+</button>
              <button
                onclick={() => deletePreset(headerPresetId)}
                disabled={!headerPresetId}
                title="Delete this preset"
                class="px-2 py-1 text-[12px] border border-[var(--border)] rounded
                       text-[var(--text-secondary)] hover:text-red-400 hover:border-red-500
                       transition-colors leading-none disabled:opacity-30 disabled:cursor-not-allowed"
              >−</button>
            </div>
          {/if}
        {/if}
      </div>

      <!-- Options content -->
      <div class="flex-1 min-h-0 overflow-y-auto p-4">
        {#if !globalOutputFormat || outputPickerOpen}
          <!-- ── Format picker: sectioned list with columns ─────────────────── -->
          <div class="space-y-4">
            {#each FORMAT_GROUPS as group}
              <div>
                <div class="flex items-center gap-2 mb-1.5">
                  <span class="text-[9px] font-semibold uppercase tracking-wider text-[var(--text-secondary)]">
                    {group.label}
                  </span>
                  <div class="flex-1 h-px bg-[var(--border)]"></div>
                </div>
                <div class="flex flex-wrap gap-1">
                  {#each group.fmts.filter(f => !f.todo || import.meta.env.DEV) as f}
                    {@const incompatible = compatibleOutputCats !== null && !compatibleOutputCats.includes(group.cat)}
                    <button
                      onclick={() => { if (!incompatible) { globalOutputFormat = f.id; outputPickerOpen = false; } }}
                      class="px-2 py-0.5 rounded text-[11px] font-mono border transition-colors
                             {incompatible ? 'opacity-25 cursor-default' : ''}
                             {globalOutputFormat === f.id
                               ? 'bg-[var(--accent)] text-white border-[var(--accent)]'
                               : f.todo
                                 ? 'border-green-900 text-green-400 hover:border-green-600 hover:bg-green-950'
                                 : 'border-[var(--border)] text-[var(--text-primary)] hover:border-[var(--accent)] hover:text-[var(--accent)]'}"
                    >{f.label ?? f.id}</button>
                  {/each}
                </div>
              </div>
            {/each}
          </div>
        {:else if activeOutputCategory === 'image'}
          <ImageOptions bind:options={imageOptions}
            onqualitystart={onQualityStart}
            onqualityinput={onQualityInput}
            oncropstart={activateCrop}
            oncropclear={clearCropValues}
            cropActive={cropActive}
            cropAspect={cropAspect}
          />
        {:else if activeOutputCategory === 'video'}
          <VideoOptions bind:options={videoOptions} errors={validationErrors} />
        {:else if activeOutputCategory === 'audio'}
          <AudioOptions bind:options={audioOptions} errors={validationErrors} />
        {:else if activeOutputCategory === 'data'}
          <DataOptions bind:options={dataOptions} />
        {:else if activeOutputCategory === 'document'}
          <DocumentOptions bind:options={documentOptions} />
        {:else if activeOutputCategory === 'archive'}
          <ArchiveOptions bind:options={archiveOptions} toolWarnings={toolWarnings} />
        {:else}
          <div class="flex flex-col items-center justify-center h-full text-center gap-2">
            <p class="text-[11px] text-green-500">Coming soon</p>
          </div>
        {/if}
      </div>

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

<style>
  @keyframes preview-slide {
    0%   { left: -33%; }
    50%  { left: 50%; }
    100% { left: 133%; }
  }
  .preview-loading-bar {
    animation: preview-slide 1.2s ease-in-out infinite;
  }
</style>
