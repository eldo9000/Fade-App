<script>
  import { invoke, convertFileSrc } from '@tauri-apps/api/core';
  import fadeIconDark from '../Fade-icon-dark.png';
  import { listen } from '@tauri-apps/api/event';
  import { onMount, onDestroy } from 'svelte';
  import { initTheme } from '@libre/ui/src/theme.js';
  import { ProgressBar } from '@libre/ui';
  import QueueManager from './lib/QueueManager.svelte';
  import Timeline from './lib/Timeline.svelte';
  import ImageOptions from './lib/ImageOptions.svelte';
  import VideoOptions from './lib/VideoOptions.svelte';
  import CodecWarning from './lib/CodecWarning.svelte';
  import AudioOptions from './lib/AudioOptions.svelte';
  import DataOptions from './lib/DataOptions.svelte';
  import FormatPicker from './lib/FormatPicker.svelte';
  import ArchiveOptions from './lib/ArchiveOptions.svelte';
  import { validateOptions } from './lib/utils.js';
  import { markConverting, markError, markDone, markCancelled, applyProgressIfActive } from './lib/itemStatus.js';
  import { createLimiter, defaultBatchConcurrency } from './lib/concurrency.js';
  import { createZoom, ZOOM_STEPS } from './lib/stores/zoom.svelte.js';
  import { tooltip, setHint } from './lib/stores/tooltip.svelte.js';
  import { overlay } from './lib/stores/overlay.svelte.js';

  // Move a rendered element to document.body so it escapes every ancestor
  // stacking context / overflow / transform. Used by the overlay dropdown.
  function portal(node) {
    document.body.appendChild(node);
    return () => { if (node.parentNode === document.body) document.body.removeChild(node); };
  }
  import { getCurrentWindow } from '@tauri-apps/api/window';
  import { createSettings } from './lib/stores/settings.svelte.js';
  import { pushError, clearDiagnostics, getEntries as getDiagEntries, snapshotText as diagSnapshot, loadPersisted as loadPersistedDiag, uploadDiagnostics, BETA } from './lib/stores/diagnostics.svelte.js';
  import { getVersion } from '@tauri-apps/api/app';
  import UpdateManager from './lib/UpdateManager.svelte';
  import PresetManager from './lib/PresetManager.svelte';
  import CropEditor from './lib/CropEditor.svelte';
  import ChromaKeyPanel from './lib/ChromaKeyPanel.svelte';
  import AnalysisTools from './lib/AnalysisTools.svelte';
  import OperationsPanel from './lib/OperationsPanel.svelte';

  const DOCUMENT_FORMATS = ['html', 'md', 'txt', 'pdf', 'docx'];
  const ARCHIVE_FORMATS = ['zip', 'tar.gz', 'tar.xz', '7z'];
  // Premiere XML (`.xml`) is deferred — extension clashes with data XML.
  const TIMELINE_FORMATS = ['otio', 'edl', 'fcpxml', 'aaf'];
  const FONT_FORMATS = ['ttf', 'otf', 'woff', 'woff2'];
  // Subtitle: ffmpeg handles srt↔vtt↔ass↔ssa natively. sbv and ttml
  // stay `todo` until we have a fallback path (ffmpeg can't read sbv
  // and can only write ttml).
  const SUBTITLE_FORMATS = ['srt', 'vtt', 'ass', 'ssa'];
  const EBOOK_FORMATS = ['epub', 'mobi', 'azw3', 'fb2', 'lit'];
  const EMAIL_FORMATS = ['eml', 'mbox'];
  // Jupyter notebook outputs (input ext `.ipynb` is routed backend-side).
  const NOTEBOOK_FORMATS = ['md', 'py', 'html'];

  // Fade is a raw utility, not a playback viewer — performance beats fidelity.
  // Standard preview is already half-resolution. Items flip to draft mode
  // automatically when EITHER threshold trips:
  //   - file size  > 500 MB  (catches big video + lossless audio)
  //   - duration   > 30 min  (catches long recordings regardless of bitrate —
  //                           decode time scales with runtime, not bytes)
  // Either condition is enough: a 45-min 256kbps lecture is ~85 MB but its
  // waveform pass is still a 45-min decode. There is intentionally no manual
  // override; a "DRAFT" badge surfaces the mode to the user and that's it.
  const HEAVY_FILE_BYTES = 500 * 1024 * 1024;  // 500 MB
  const HEAVY_DURATION_SECS = 30 * 60;         // 30 min

  function isHeavyItem(item) {
    const info = item?.info;
    if (!info) return false;
    if ((info.file_size ?? 0) > HEAVY_FILE_BYTES) return true;
    if ((info.duration_secs ?? 0) > HEAVY_DURATION_SECS) return true;
    return false;
  }

  // ── State ──────────────────────────────────────────────────────────────────

  const zoom = createZoom();

  // ── QueueManager component ref ────────────────────────────────────────────
  // Queue state, selection, drag, and the async preview pipeline all live in
  // QueueManager.svelte. App.svelte binds to the outputs it needs.
  let queueManagerEl = $state(null);

  // $bindable outputs from QueueManager — read/write by App.svelte
  let queue          = $state([]);
  let selectedItem   = $state(null);
  let selectedMediaType = $state(null);
  let selectedIds    = $state(new Set());
  let draggingFileId = $state(null);

  // ── Operations mode ────────────────────────────────────────────────────────
  let operationsMode = $state(false);
  let selectedOperation = $state(null);
  // replaceAudioPath is $bindable in OperationsPanel; App reads it for Timeline.
  let replaceAudioPath = $state(null);
  // Chroma Key state is now owned by ChromaKeyPanel.svelte (via OperationsPanel).
  // Analysis state is now owned by AnalysisTools.svelte (via OperationsPanel).
  // All other operations state is now owned by OperationsPanel.svelte.
  const OPERATIONS = [
    { id: 'cut-noenc',      label: 'Cut/Extract' },
    { id: 'replace-audio',  label: 'Replace Audio' },
    { id: 'rewrap',         label: 'Rewrap' },
    { id: 'conform',        label: 'Conform' },
    { id: 'merge',          label: 'Merge' },
    { id: 'extract',        label: 'Extract' },
    { id: 'subtitling',     label: 'Subtitling' },
    { id: 'video-inserts',  label: 'Video Inserts' },
    { id: 'silence-remove', label: 'Silence Remover' },
    // Intact stream-copy tools (group 1).
    { id: 'remove-audio',   label: 'Remove Audio' },
    { id: 'remove-video',   label: 'Remove Video' },
    { id: 'metadata-strip', label: 'Strip Metadata' },
    { id: 'loop',           label: 'Loop' },
    // Video processing filters (group 2).
    { id: 'rotate-flip',    label: 'Rotate / Flip' },
    { id: 'speed',          label: 'Speed / Reverse' },
    { id: 'fade',           label: 'Crop / Fade' },
    { id: 'deinterlace',    label: 'Deinterlace' },
    { id: 'denoise',        label: 'Denoise' },
    // Frame / image tools (group 3).
    { id: 'thumbnail',      label: 'Thumbnail' },
    { id: 'contact-sheet',  label: 'Contact Sheet' },
    { id: 'frame-export',   label: 'Frame Export' },
    { id: 'watermark',      label: 'Watermark' },
    // Audio processing (group 4).
    { id: 'channel-tools',  label: 'Channel Tools' },
    { id: 'pad-silence',    label: 'Pad Silence' },
    // Chroma key — tier 1 (FFmpeg built-ins).
    { id: 'chroma-ffmpeg',  label: 'Chroma Key (FFmpeg)' },
    // Analysis ops — reports, no output file.
    { id: 'loudness',       label: 'Loudness & True Peak' },
    { id: 'audio-norm',     label: 'Audio Normalize' },
    { id: 'cut-detect',     label: 'Cut Detection' },
    { id: 'black-detect',   label: 'Black Detection' },
    { id: 'vmaf',           label: 'VMAF' },
    { id: 'framemd5',       label: 'FrameMD5' },
  ];
  function enterOperation(id) { selectedOperation = id; operationsMode = true; }
  function exitOperationsMode() { operationsMode = false; selectedOperation = null; }

  let converting = $state(false);
  let paused = $state(false);

  // Two independent progress signals stacked in the bottom panel:
  //   currentPercent — % of the active job (averaged if >1 in flight; usually 1)
  //   overallPercent — count of terminated items in the CURRENT BATCH / batch size × 100
  // A "batch" is one click of Convert Selected/All. Without batching, overall
  // would stay pinned at 100% forever once any item finishes, because every
  // newly-submitted item becomes terminal immediately (fast data conversions
  // flip 'pending' → 'done' in one event-loop tick with no visible interim).
  let batchIds = $state(new Set());
  // Per-job completion resolvers. `convert_file` returns immediately after
  // spawning its worker thread, so the `invoke` promise can't gate the
  // batch limiter. Instead, each dispatched job parks here until the
  // matching job-done/job-error/job-cancelled event fires (or the IPC call
  // itself rejects before the thread starts).
  /** @type {Map<string, () => void>} */
  const batchCompletions = new Map();
  function resolveBatchCompletion(jobId) {
    const fn = batchCompletions.get(jobId);
    if (fn) {
      batchCompletions.delete(jobId);
      fn();
    }
  }
  let currentPercent = $derived.by(() => {
    const active = queue.filter(q => q.status === 'converting');
    if (active.length === 0) return 0;
    return active.reduce((s, q) => s + (q.percent ?? 0), 0) / active.length;
  });
  let overallPercent = $derived.by(() => {
    if (batchIds.size === 0) return 0;
    const batch = queue.filter(q => batchIds.has(q.id));
    if (batch.length === 0) return 0;
    const terminal = batch.filter(q =>
      q.status === 'done' || q.status === 'error' || q.status === 'cancelled'
    ).length;
    return (terminal / batch.length) * 100;
  });

  let statusMessage = $state('');
  // 'info' | 'success' | 'error' — drives the colour of the status box text.
  // Gray by default; green for success states; red for things the user must see.
  let statusKind    = $state('info');
  function setStatus(text, kind = 'info') { statusMessage = text; statusKind = kind; }
  let validationErrors = $state({});
  let toolWarnings = $state({});

  // ── Auto-updater ───────────────────────────────────────────────────────────
  // State is owned by UpdateManager; App.svelte binds to updateState so the
  // tooltip bar notification can read it.
  let updateState = $state('idle');
  let updateManagerEl = $state(null);

  /** Piggyback diagnostics upload onto the update action. Runs before the
   *  actual download/open so the user has already consented to this server
   *  transaction. Never blocks — upload has its own 5s timeout and silent
   *  failure. During beta the toggle is effectively forced on. */
  async function _uploadBeforeUpdate() {
    const enabled = BETA || !!settings.sendDiagnostics;
    await uploadDiagnostics({
      enabled,
      meta: {
        version: appVersion,
        userAgent: typeof navigator !== 'undefined' ? navigator.userAgent : '',
      },
    });
  }

  // App version — read from Cargo manifest via Tauri at runtime so the footer
  // and diagnostics report stay in sync with what actually shipped.
  let appVersion = $state('');
  // Diagnostics panel disclosure — closed by default so the errors list
  // doesn't dominate the small Settings popover.
  let diagnosticsExpanded = $state(false);
  const diagEntries = getDiagEntries();

  // ── Diagnostics helpers ────────────────────────────────────────────────────
  let _diagCopyNote = $state('');

  function _diagHeader() {
    const ua = typeof navigator !== 'undefined' ? navigator.userAgent : 'unknown';
    return [
      `Fade v${appVersion || '?'}`,
      `User agent: ${ua}`,
      `Captured: ${new Date().toISOString()}`,
      `Entries: ${diagEntries.length}`,
    ].join('\n');
  }

  async function copyDiagnostics() {
    try {
      await navigator.clipboard.writeText(diagSnapshot(_diagHeader()));
      _diagCopyNote = 'Copied to clipboard';
      setTimeout(() => { _diagCopyNote = ''; }, 1800);
    } catch (e) {
      pushError('diagnostics', 'Could not copy to clipboard', e);
      _diagCopyNote = 'Copy failed';
      setTimeout(() => { _diagCopyNote = ''; }, 1800);
    }
  }

  // ── Preview pipeline state — owned by QueueManager, bound here ──────────────
  // QueueManager.runLoadPipeline writes these; App.svelte template reads them.
  let selectedDuration       = $state(null);
  let selectedWidth          = $state(null);
  let selectedHeight         = $state(null);
  let previewLoading         = $state(false);
  let liveSrc                = $state(null);
  let tlMediaReady           = $state(false);
  let tlWaveformReady        = $state(false);
  let tlSpectrogramReady     = $state(false);
  let tlFilmstripReady       = $state(false);
  let cachedWaveformForTimeline  = $state(null);
  let cachedFilmstripForTimeline = $state(null);

  function onPreviewLoaded() { previewLoading = false; }
  function onVideoMetaLoaded() {
    previewLoading = false;
    // Seek to first real frame — prevents "black preview" for videos that open on black
    if (videoEl) videoEl.currentTime = 0.001;
  }

  /** Called by QueueManager when the user selects a new item. App.svelte clears
   *  preview state synchronously so the browser paints a blank frame before the
   *  async pipeline fills it in. Also handles vizExpanded auto-expand. */
  function onSelectionChange(newItem, { isMedia }) {
    settingsOpen = false;
    liveSrc            = null;
    selectedDuration   = null;
    selectedWidth      = null;
    selectedHeight     = null;
    tlMediaReady       = false;
    tlWaveformReady    = false;
    tlSpectrogramReady = false;
    tlFilmstripReady   = false;
    previewLoading     = isMedia;
    videoEl?.load();   // flash existing video to black immediately

    // Auto-expand viz based on setting — only ever set to true, never force-collapse.
    const vd = settings.vizDefault;
    const shouldExpand = newItem?.mediaType === 'video'
      ? vd === 'av'
      : newItem?.mediaType === 'audio'
        ? vd === 'audio' || vd === 'av'
        : false;
    if (shouldExpand) vizExpanded = true;
  }

  // File info dialog

  // Browse file input
  let fileInput = $state(null);

  // Preview video element — bound so Timeline can drive it
  let videoEl = $state(null);

  // Advanced audio panel — persists across file switches
  let vizExpanded = $state(false);
  let queueCompact = $state(false);
  const recurseSubfolders = true;

  // Sidebar filters — typed into the search field at the top of each
  // sidebar, narrows what's visible below. Case-insensitive substring.
  let leftSearch = $state('');
  let rightSearch = $state('');

  // ── Settings ───────────────────────────────────────────────────────────────
  const settings   = createSettings();
  let settingsOpen = $state(false);
  let aboutOpen = $state(false);
  let aboutClosing = $state(false);
  function closeAbout() {
    if (aboutClosing) return;
    aboutClosing = true;
    setTimeout(() => { aboutOpen = false; aboutClosing = false; }, 500);
  }

  // Auto-collapse queue once per session when it grows large enough to scroll
  let _autoCompactDone = false;
  $effect(() => {
    if (!settings.autoCompact || _autoCompactDone || queueCompact) return;
    if (queue.length > 12) { queueCompact = true; _autoCompactDone = true; }
  });

  // Compression diff preview
  let diffClipPath   = $state(null);
  let diffLoading    = $state(false);
  let diffError      = $state(null);
  let diffNote       = $state(null);
  let _diffPreviewGen = 0;
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
  let _imgDiffGen       = 0;
  let _qualityDragging  = false;

  function _clearImageDiff() {
    imgDiffMode = false; imgDiffPath = null; imgCompressedPath = null;
  }

  async function _runImageDiff() {
    if (!selectedItem || selectedItem.mediaType !== 'image') return;
    const gen = ++_imgDiffGen;
    imgDiffLoading = true;
    try {
      const result = await invoke('preview_image_quality', {
        path: selectedItem.path,
        quality: imageOptions.quality,
        outputFormat: imageOptions.output_format,
      });
      if (gen !== _imgDiffGen) return;  // stale — newer slider tick in flight
      imgDiffPath       = result.diff_path;
      imgCompressedPath = result.compressed_path;
    } catch { /* non-fatal — lossless format or magick missing */ }
    finally { if (gen === _imgDiffGen) imgDiffLoading = false; }
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
  $effect(() => { selectedItem?.id; _clearImageDiff(); });
  $effect(() => { imageOptions.output_format; _clearImageDiff(); });

  // ── Crop state ─────────────────────────────────────────────────────────────
  // Owned by CropEditor; App.svelte holds refs for passing into the component.
  let previewAreaEl  = $state(null);
  let imgEl          = $state(null);
  let imgNaturalW    = $state(0);
  let imgNaturalH    = $state(0);
  let cropActive     = $state(false);
  let cropAspect     = $state(null);
  let cropEditorEl   = $state(null);

  function onImgLoad(e) {
    imgNaturalW = e.currentTarget.naturalWidth;
    imgNaturalH = e.currentTarget.naturalHeight;
  }

  // Clear any diff preview when the selected file changes
  $effect(() => {
    selectedItem?.id;
    diffClipPath = null; diffError = null; diffNote = null;
  });

  async function runDiffPreview() {
    if (!selectedItem || selectedItem.mediaType !== 'video') return;
    const at = videoEl?.currentTime ?? 0;
    try { videoEl?.pause(); } catch { /* non-fatal */ }
    const gen = ++_diffPreviewGen;
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
      if (gen !== _diffPreviewGen) return;  // stale — newer diff request in flight
      diffClipPath = result.path;
      diffNote = result.note;
    } catch (e) {
      if (gen !== _diffPreviewGen) return;
      diffError = String(e);
    } finally {
      if (gen === _diffPreviewGen) diffLoading = false;
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
    // ── Format-specific ──
    jpeg_chroma: '420',
    jpeg_progressive: false,
    png_compression: 6,
    png_color_mode: 'rgba',
    png_interlaced: false,
    tiff_compression: 'lzw',
    tiff_bit_depth: 8,
    tiff_color_mode: 'rgb',
    webp_lossless: false,
    webp_method: 4,
    avif_speed: 6,
    avif_chroma: '420',
    bmp_bit_depth: 24,
    preserve_metadata: true,
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
    // ── Format-specific ──
    crf: 20,
    preset: 'medium',
    h264_profile: 'high',
    pix_fmt: 'yuv420p',
    tune: 'none',
    frame_rate: 'original',
    webm_bitrate_mode: 'crf',
    av1_speed: 8,
    vp9_speed: 1,
    mkv_subtitle: 'copy',
    avi_video_bitrate: 4000,
    gif_width: 480,
    gif_fps: 10,
    gif_loop: 'infinite',
    gif_palette_size: 256,
    gif_dither: 'floyd',
    preserve_metadata: true,
    output_dir: null,
    // ── Professional codec defaults ──
    hap_format: 'hap',
    dnxhr_profile: 'dnxhr_sq',
    dnxhd_bitrate: 185,
    dv_standard: 'ntsc',
    video_bitrate_mode: 'crf',
    video_bitrate: 4000,
    prores_profile: 3,
  });

  let audioOptions = $state({
    output_format: 'mp3',
    bitrate: 192,
    sample_rate: 44100,
    normalize_loudness: false,
    trim_start: null,
    trim_end: null,
    fade_in: null,
    fade_out: null,
    pad_front: null,
    pad_end: null,
    dsp_highpass_freq: null,
    dsp_lowpass_freq: null,
    dsp_stereo_width: null,
    dsp_limiter_db: null,
    // ── Format-specific ──
    channels: 'source',
    bit_depth: 16,
    mp3_bitrate_mode: 'cbr',
    mp3_vbr_quality: 2,
    flac_compression: 5,
    ogg_bitrate_mode: 'vbr',
    ogg_vbr_quality: 5,
    aac_profile: 'lc',
    opus_application: 'audio',
    opus_vbr: true,
    m4a_subcodec: 'aac',
    wma_mode: 'standard',
    ac3_bitrate: 448,
    dts_bitrate: 1510,
    preserve_metadata: true,
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
    archive_compression: 5,
    output_dir: null,
  });

  // 3D models are converted via the assimp CLI. No user-tunable options yet —
  // format-ID selection happens backend-side from `output_format`.
  let modelOptions = $state({
    output_format: 'glb',
    output_dir: null,
  });

  // Timeline / EDL conversions — shelled out to OpenTimelineIO's
  // `otioconvert`. Format picked by output extension; no other options.
  let timelineOptions = $state({
    output_format: 'otio',
    output_dir: null,
  });

  // Font conversions — shelled out to fonttools (Python). Output flavor
  // picked by extension (ttf/otf/woff/woff2). No other options today.
  let fontOptions = $state({
    output_format: 'woff2',
    output_dir: null,
  });

  let subtitleOptions = $state({
    output_format: 'vtt',
    output_dir: null,
  });

  let ebookOptions = $state({
    output_format: 'epub',
    output_dir: null,
  });

  let emailOptions = $state({
    output_format: 'mbox',
    output_dir: null,
  });

  // ── Event listeners ────────────────────────────────────────────────────────

  let unlistenProgress, unlistenDone, unlistenError, unlistenCancelled;

  onMount(async () => {
    // Hydrate diagnostics from disk before installing handlers so historical
    // errors show alongside new ones in the Diagnostics panel.
    await loadPersistedDiag();

    // Global error capture — installed before anything else so early failures
    // get logged. Errors go to the in-memory diagnostics ring buffer (mirrored
    // to a JSONL file under the app-log dir); nothing leaves the machine.
    window.addEventListener('error', (ev) => {
      pushError('window.onerror', ev.message || 'Uncaught error',
                ev.error?.stack ?? `${ev.filename ?? '?'}:${ev.lineno ?? '?'}`);
    });
    window.addEventListener('unhandledrejection', (ev) => {
      const reason = ev.reason;
      const msg = reason?.message ?? String(reason);
      pushError('unhandledrejection', msg, reason?.stack);
    });

    try { appVersion = await getVersion(); } catch { /* non-fatal */ }

    await initTheme(invoke);

    // Restore zoom and wire hotkeys
    document.documentElement.style.zoom = String(zoom.level);
    window.addEventListener('keydown', zoom.handleKey);

    // Mouse back button (X1) → click the first on-screen [data-back-button].
    // Preempt in mousedown so the webview doesn't attempt history nav first;
    // auxclick covers browsers that only surface X1 there.
    const handleMouseBack = (ev) => {
      if (ev.button !== 3) return;
      const btn = document.querySelector('[data-back-button]');
      if (!btn) return;
      ev.preventDefault();
      ev.stopPropagation();
      btn.click();
    };
    window.addEventListener('mousedown', handleMouseBack);
    window.addEventListener('auxclick', handleMouseBack);

    // Pre-load Test-Files folder
    try {
      const testDir = '/Users/eldo/Desktop/Test-Files';
      const files = await invoke('scan_dir', { path: testDir, recursive: recurseSubfolders });
      if (files.length > 0) {
        queueManagerEl?.addFiles(files);
        imageOptions.output_dir = testDir;
        videoOptions.output_dir = testDir;
        audioOptions.output_dir = testDir;
        dataOptions.output_dir = testDir;
        documentOptions.output_dir = testDir;
        archiveOptions.output_dir = testDir;
      }
    } catch { /* non-fatal — folder may not exist */ }

    // Note: bg filmstrip listener now lives in QueueManager.$effect

    unlistenProgress = await listen('job-progress', ({ payload }) => {
      const item = queue.find(q => q.id === payload.job_id);
      if (!item) return;
      // Terminal states win: a late progress line from ffmpeg stderr must
      // not flip a cancelled/done/error item back to 'converting'.
      if (!applyProgressIfActive(item, payload.percent)) return;
      setStatus(payload.message, 'info');
    });

    unlistenDone = await listen('job-done', ({ payload }) => {
      const item = queue.find(q => q.id === payload.job_id);
      if (item && item.status !== 'cancelled') markDone(item, payload.output_path);
      resolveBatchCompletion(payload.job_id);
      checkAllDone();
    });

    unlistenError = await listen('job-error', ({ payload }) => {
      const item = queue.find(q => q.id === payload.job_id);
      if (item && item.status !== 'cancelled') {
        markError(item, payload.message);
        pushError('job', `Conversion failed: ${item.name ?? payload.job_id}`, payload.message);
      }
      resolveBatchCompletion(payload.job_id);
      checkAllDone();
    });

    unlistenCancelled = await listen('job-cancelled', ({ payload }) => {
      const item = queue.find(q => q.id === payload.job_id);
      if (item) markCancelled(item);
      resolveBatchCompletion(payload.job_id);
      checkAllDone();
    });

    presetManagerEl?.loadPresets();
    checkTools();

    // Fire-and-forget update check after a short delay so startup isn't blocked.
    setTimeout(() => { updateManagerEl?.maybeCheckForUpdate(); }, 2000);
  });

  onDestroy(() => {
    window.removeEventListener('keydown', zoom.handleKey);
    // Note: bg filmstrip listener cleaned up by QueueManager.$effect teardown
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
      const missing = Object.entries(toolWarnings).filter(([, m]) => m).map(([k]) => k);
      if (missing.length > 0) pushError('tools', `Missing external tool(s): ${missing.join(', ')}`);
    } catch (e) {
      pushError('tools', 'check_tools invoke failed', e);
    }
  }

  let dismissedWarnings = $state(new Set());

  function dismissWarning(tool) {
    const next = new Set(dismissedWarnings);
    next.add(tool);
    dismissedWarnings = next;
  }

  // ── Helpers ────────────────────────────────────────────────────────────────

  function checkAllDone() {
    const active = queue.filter(q => q.status === 'converting');
    if (active.length === 0) {
      converting = false;
      paused = false;
      // Summarise the CURRENT batch (not the whole queue), matching the
      // user's mental model: "the thing I just clicked Convert on is done".
      const batch = queue.filter(q => batchIds.has(q.id));
      const total = batch.length;
      const done = batch.filter(q => q.status === 'done').length;
      const cancelled = batch.filter(q => q.status === 'cancelled').length;
      const errored = batch.filter(q => q.status === 'error').length;

      if (total === 0) {
        setStatus('', 'info');
      } else if (errored === 0 && cancelled === 0) {
        setStatus(total === 1 ? '1 file done' : `${total} files done`, 'success');
      } else {
        const parts = [];
        if (done) parts.push(`${done} of ${total} done`);
        if (errored) parts.push(`${errored} failed`);
        if (cancelled) parts.push(`${cancelled} cancelled`);
        // Red if anything failed; otherwise (only cancellations) keep it gray.
        setStatus(parts.join(', '), errored > 0 ? 'error' : 'info');
      }
    }
  }

  // addFiles / removeItem / addBatchFolder / moveItemToFolder / toggleFolderExpanded
  // live in QueueManager — call via queueManagerEl.methodName(...)

  /** Wrapper: clears the queue via QueueManager then resets App-level op state. */
  function clearQueue() {
    queueManagerEl?.clearQueue();
    converting = false;
    paused = false;
    validationErrors = {};
    batchIds = new Set();
  }

  // ── Browse ─────────────────────────────────────────────────────────────────

  let _proxyAddTarget = null;

  function onBrowse() { fileInput?.click(); }

  function onProxyBrowse(folderId) {
    _proxyAddTarget = folderId;
    fileInput?.click();
  }

  function onFileInputChange(e) {
    const paths = Array.from(e.target.files ?? []).map(f => f.path ?? f.name);
    if (paths.length) queueManagerEl?.addFiles(paths, _proxyAddTarget);
    _proxyAddTarget = null;
    e.target.value = '';
  }

  // ── Auxiliary single-file picker ───────────────────────────────────────────
  // Shared hidden <input> that ops pages (VMAF reference, subtitle diff, etc.)
  // drive through pickAuxFile(setter). Replaces the earlier 'pending-pick'
  // placeholder strings. Phase 2 wiring can swap this for the richer
  // plugin-dialog API if/when we add that dep.
  let auxFileInput = $state(null);
  let _auxFileTarget = null;
  function pickAuxFile(setter, accept = '') {
    _auxFileTarget = setter;
    if (!auxFileInput) return;
    auxFileInput.accept = accept;
    auxFileInput.click();
  }
  function onAuxFileChange(e) {
    const f = e.target.files?.[0];
    if (f && _auxFileTarget) _auxFileTarget(f.path ?? f.name);
    _auxFileTarget = null;
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
    else { paused = true; setStatus('Paused — click Resume to continue', 'info'); }
  }



  // ── Convert ────────────────────────────────────────────────────────────────

  // Mirrors src-tauri/src/lib.rs::build_output_path so we can check collisions
  // client-side before invoking convert_file. Keep them in sync.
  function expectedOutputPath(item, newExt, suffix, outputDirOverride, sep = '_') {
    const lastSlash = item.path.lastIndexOf('/');
    const parentDir = lastSlash >= 0 ? item.path.slice(0, lastSlash) : '.';
    const dir = outputDirOverride ?? parentDir;
    const stem = item.ext ? item.name.slice(0, -(item.ext.length + 1)) : item.name;
    return suffix
      ? `${dir}/${stem}${sep}${suffix}.${newExt}`
      : `${dir}/${stem}.${newExt}`;
  }

  // Operations functions are now owned by OperationsPanel.svelte.

  async function startConvert(mode = 'all') {
    const errors = validateOptions(videoOptions, audioOptions);
    if (Object.keys(errors).length > 0) { validationErrors = errors; return; }
    validationErrors = {};

    if (!globalOutputFormat) {
      setStatus('Select an output format first', 'error');
      return;
    }

    const candidates = mode === 'selected'
      ? (selectedIds.size > 1
          ? queue.filter(q => selectedIds.has(q.id))
          : selectedItem ? [selectedItem] : [])
      : visibleQueue;

    // Allow re-converting done / error / cancelled items. Only skip items
    // that are actively converting right now.
    const eligible = candidates.filter(q => q.status !== 'converting');
    const compat = compatibleTypes;
    const compatible = eligible.filter(q => compat.includes(q.mediaType));
    const skipped = eligible.length - compatible.length;
    if (compatible.length === 0) {
      setStatus(skipped > 0
        ? `No compatible files — ${skipped} skipped (incompatible)`
        : 'No files to convert', 'error');
      return;
    }

    // Pre-flight: skip items whose output file already exists. User must
    // delete the old output manually to re-run (avoids silent overwrite).
    const outExt = globalOutputFormat;
    const checked = await Promise.all(compatible.map(async item => ({
      item,
      exists: await invoke('file_exists', {
        path: expectedOutputPath(item, outExt, outputSuffix, outputDir, outputSeparator),
      }).catch(() => false),
    })));
    const willRun     = checked.filter(c => !c.exists).map(c => c.item);
    const alreadyDone = checked.filter(c =>  c.exists).length;
    if (willRun.length === 0) {
      setStatus(alreadyDone === 1 ? 'File already exists.' : 'Files already exist.', 'error');
      return;
    }

    // Start a fresh batch if the previous one already finished. If the user
    // clicks Convert again while jobs are still in flight, items pile onto
    // the current batch instead (progress scales with total).
    const next = converting ? new Set(batchIds) : new Set();
    for (const item of willRun) next.add(item.id);
    batchIds = next;

    converting = true;
    paused = false;
    const parts = [];
    if (skipped)     parts.push(`${skipped} incompatible`);
    if (alreadyDone) parts.push(`${alreadyDone} already exist${alreadyDone === 1 ? 's' : ''}`);
    setStatus(parts.length ? `Converting… — skipped ${parts.join(', ')}` : 'Converting…', 'info');

    // Cap concurrent ffmpeg fanout. Without this, a 100-item batch would
    // launch 100 concurrent ffmpegs and thrash the CPU.
    const limiter = createLimiter(defaultBatchConcurrency());
    for (const item of willRun) {
      if (paused) break;

      const opts = item.mediaType === 'image'    ? { ...imageOptions,    output_suffix: outputSuffix, output_separator: outputSeparator, output_dir: outputDir }
             : item.mediaType === 'video'    ? { ...videoOptions,    output_suffix: outputSuffix, output_separator: outputSeparator, output_dir: outputDir }
             : item.mediaType === 'audio'    ? { ...audioOptions,    output_suffix: outputSuffix, output_separator: outputSeparator, output_dir: outputDir }
             : item.mediaType === 'data'     ? { ...dataOptions,     output_suffix: outputSuffix, output_separator: outputSeparator, output_dir: outputDir }
             : item.mediaType === 'document' ? { ...documentOptions, output_suffix: outputSuffix, output_separator: outputSeparator, output_dir: outputDir }
             : item.mediaType === 'archive'  ? { ...archiveOptions,  output_suffix: outputSuffix, output_separator: outputSeparator, output_dir: outputDir }
             : item.mediaType === 'model'    ? { ...modelOptions,    output_suffix: outputSuffix, output_separator: outputSeparator, output_dir: outputDir }
             : item.mediaType === 'timeline' ? { ...timelineOptions, output_suffix: outputSuffix, output_separator: outputSeparator, output_dir: outputDir }
             : item.mediaType === 'subtitle' ? { ...subtitleOptions, output_suffix: outputSuffix, output_separator: outputSeparator, output_dir: outputDir }
             : item.mediaType === 'ebook'    ? { ...ebookOptions,    output_suffix: outputSuffix, output_separator: outputSeparator, output_dir: outputDir }
             : item.mediaType === 'email'    ? { ...emailOptions,    output_suffix: outputSuffix, output_separator: outputSeparator, output_dir: outputDir }
             :                                 { ...fontOptions,     output_suffix: outputSuffix, output_separator: outputSeparator, output_dir: outputDir };

      limiter.run(() => {
        // Re-check at slot-acquire time: user may have paused / cancelled / cleared
        // the queue while this task was waiting for a free slot.
        if (paused || item.status === 'cancelled' || !batchIds.has(item.id)) {
          return Promise.resolve();
        }
        markConverting(item);
        // `convert_file` spawns a worker thread and returns immediately, so
        // we can't use the invoke promise to gate the slot. Park on a
        // completion resolver that the job-done/error/cancelled listeners
        // fire when the worker finishes.
        const done = new Promise(resolve => batchCompletions.set(item.id, resolve));
        invoke('convert_file', { jobId: item.id, inputPath: item.path, options: opts })
          .catch(err => {
            // IPC-level failure before the worker thread starts — no event
            // will fire, so release the slot ourselves.
            markError(item, err);
            resolveBatchCompletion(item.id);
            checkAllDone();
          });
        return done;
      });
    }
  }

  // ── Edit detection — has a category's options deviated from identity? ────────
  // Used to show an edit indicator on queue items (dash + italic filename).
  const _audioHasEdits = $derived(
    (audioOptions.trim_start != null && audioOptions.trim_start > 0) ||
    audioOptions.trim_end != null ||
    (audioOptions.fade_in  != null && audioOptions.fade_in  > 0) ||
    (audioOptions.fade_out != null && audioOptions.fade_out > 0) ||
    audioOptions.normalize_loudness ||
    audioOptions.dsp_highpass_freq  != null ||
    audioOptions.dsp_lowpass_freq   != null ||
    audioOptions.dsp_stereo_width   != null ||
    audioOptions.dsp_limiter_db     != null ||
    (audioOptions.pad_front != null && audioOptions.pad_front > 0) ||
    (audioOptions.pad_end   != null && audioOptions.pad_end   > 0)
  );
  const _videoHasEdits = $derived(
    (videoOptions.trim_start != null && videoOptions.trim_start > 0) ||
    videoOptions.trim_end != null ||
    videoOptions.frame_rate !== 'original'
  );
  const _imageHasEdits = $derived(
    imageOptions.crop_width != null ||
    imageOptions.rotation !== 0      ||
    imageOptions.flip_h              ||
    imageOptions.flip_v              ||
    imageOptions.resize_mode !== 'none'
  );
  function itemHasEdits(item) {
    if (!item || item.kind !== 'file') return false;
    if (!selectedIds.has(item.id)) return false;
    if (item.mediaType === 'audio') return _audioHasEdits;
    if (item.mediaType === 'video') return _videoHasEdits;
    if (item.mediaType === 'image') return _imageHasEdits;
    return false;
  }

  // ── Drag over window ───────────────────────────────────────────────────────

  let outputSuffix = $state('converted');
  let outputSeparator = $state('_');
  let outputDestMode = $state('source'); // 'source' | 'custom'
  let customOutputDir = $state(null);
  let folderInput = $state(null);
  let outputDir = $derived(outputDestMode === 'source' ? null : (customOutputDir ?? null));
  let dragOver = $state(false);

  // Hide files whose stem ends with `_<outputSuffix>` — these are prior outputs
  // that would otherwise keep getting re-converted each time Fade scans the folder.
  // Used as the source of truth for the queue display AND for Convert All.
  let visibleQueue = $derived.by(() => {
    let out = queue;
    const q = leftSearch.trim().toLowerCase();
    if (q) {
      out = out.filter(item => {
        const name = (item.name ?? '').toLowerCase();
        const ext = (item.ext ?? '').toLowerCase();
        return name.includes(q) || ext.includes(q);
      });
    }
    return out;
  });

  function onFolderInputChange(e) {
    const files = Array.from(e.target.files ?? []);
    if (files.length > 0) {
      const p = files[0].path ?? files[0].webkitRelativePath ?? '';
      const dir = p.includes('/') ? p.substring(0, p.lastIndexOf('/')) : null;
      if (dir) { customOutputDir = dir; outputDestMode = 'custom'; }
    }
    e.target.value = '';
  }

  function _isExternalFileDrag(e) {
    return !!e.dataTransfer?.types?.includes('Files');
  }
  function onWindowDragover(e) {
    if (!_isExternalFileDrag(e)) return;   // ignore intra-app row drags
    e.preventDefault();
    dragOver = true;
  }
  function onWindowDragleave(e) { if (!e.relatedTarget) dragOver = false; }
  async function onWindowDrop(e) {
    if (!_isExternalFileDrag(e)) { dragOver = false; return; }
    e.preventDefault();
    dragOver = false;
    const paths = Array.from(e.dataTransfer?.files ?? []).map(f => f.path ?? f.name);
    if (!paths.length) return;
    // Expand any dropped directories via scan_dir, honouring the Subfolders
    // toggle. `file_exists` returns true for dirs; we rely on scan_dir
    // returning [] for regular files so a single call per path works.
    const expanded = [];
    for (const p of paths) {
      try {
        const listed = await invoke('scan_dir', { path: p, recursive: recurseSubfolders });
        if (listed.length > 0) expanded.push(...listed);
        else expanded.push(p);
      } catch {
        expanded.push(p);
      }
    }
    if (expanded.length) queueManagerEl?.addFiles(expanded);
  }

  // ── Presets ────────────────────────────────────────────────────────────────
  // Owned by PresetManager; loadPresets() is called via bind:this after mount.
  let presetManagerEl  = $state(null);
  let presetsMode      = $state(false);
  let presetsList      = $state([]);

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
    else if (cat === 'model')    modelOptions.output_format    = globalOutputFormat;
    else if (cat === 'timeline') timelineOptions.output_format = globalOutputFormat;
    else if (cat === 'font')     fontOptions.output_format     = globalOutputFormat;
    else if (cat === 'subtitle') subtitleOptions.output_format = globalOutputFormat;
    else if (cat === 'ebook')    ebookOptions.output_format    = globalOutputFormat;
    else if (cat === 'email')    emailOptions.output_format    = globalOutputFormat;
  });

  // ── Tooltip bar ────────────────────────────────────────────────────────────
  function onPanelMouseOver(e) {
    const el = e.target.closest('[data-tooltip]');
    setHint(el?.dataset.tooltip ?? '');
  }

  // ── Zoom: after clicking ± or reset, warp cursor back to the button
  //    centre so rapid repeat-clicks still land on target (the button's
  //    pixel position shifts because document.documentElement.style.zoom
  //    rescales the whole UI). Uses a Tauri Rust command because browsers
  //    deny JS cursor warping. Swallows failures silently.
  async function recenterCursor(buttonEl) {
    try {
      const r = buttonEl.getBoundingClientRect();
      const cx = r.left + r.width / 2;
      const cy = r.top  + r.height / 2;
      const win   = getCurrentWindow();
      const inner = await win.innerPosition();
      const scale = await win.scaleFactor();
      // CSS px → physical screen px (zoom already baked into getBoundingClientRect)
      const sx = Math.round(inner.x + cx * scale);
      const sy = Math.round(inner.y + cy * scale);
      await invoke('set_cursor_position', { x: sx, y: sy });
    } catch { /* non-fatal */ }
  }
  function zoomClick(fn, e) {
    fn();
    // Defer one frame so zoom's style write has reflowed before we measure.
    requestAnimationFrame(() => recenterCursor(e.currentTarget));
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
      // Tracker / MIDI — rendered via fluidsynth (MIDI, requires .sf2) or
      // openmpt123 (module trackers). See src-tauri/src/convert/tracker.rs.
      // These ids are INPUT format markers in the audio category — the user
      // picks a tracker file as input, target codec is set by the audio fmt.
      { id: 'mid',  label: 'MIDI' },
      { id: 'mod',  label: 'MOD'  },
      { id: 'xm',   label: 'XM'   },
      { id: 'it',   label: 'IT'   },
      // SF2 is a soundfont container, not an audio stream; not convertible.
      { id: 'sf2',  label: 'SF2',  todo: true, preview: true },
    ]},
    { label: 'Video', cat: 'video', fmts: [
      { id: 'mp4' }, { id: 'mov' }, { id: 'webm' }, { id: 'mkv' }, { id: 'avi' }, { id: 'gif' },
      { id: 'm4v'  }, { id: 'flv'  }, { id: 'mpg'  },
      { id: 'ogv'  }, { id: 'ts'   }, { id: '3gp'  },
      { id: 'divx' }, { id: 'rmvb' }, { id: 'asf'  },
      { id: 'wmv', label: 'WMV' },
    ]},
    { label: 'Image Sequence', cat: 'seq', fmts: [
      { id: 'seq_png',  label: 'PNG',  cat: 'video' },
      { id: 'seq_jpg',  label: 'JPEG', cat: 'video' },
      { id: 'seq_tiff', label: 'TIFF', cat: 'video' },
    ]},
    // ── Codecs: quick-picks that set both the common container AND codec.
    // Clicking a codec preset drops you onto the natural container for that
    // codec (ProRes → MOV, H.264 → MP4, FFV1 → MKV, etc.) and pre-selects
    // the codec in VideoOptions.
    { label: 'Codecs', cat: 'codec', fmts: [
      { id: 'codec-h264',      label: 'H.264',         ext: 'mp4', codec: 'h264'        },
      { id: 'codec-h265',      label: 'H.265 / HEVC',  ext: 'mp4', codec: 'h265'        },
      { id: 'codec-av1',       label: 'AV1',           ext: 'mp4', codec: 'av1'         },
      { id: 'codec-vp9',       label: 'VP9',           ext: 'webm', codec: 'vp9'        },
      { id: 'codec-prores',    label: 'Apple ProRes',  ext: 'mov', codec: 'prores'                   },
      { id: 'codec-dnxhd',     label: 'DNxHD',         ext: 'mov', codec: 'dnxhd'                   },
      { id: 'codec-dnxhr',     label: 'DNxHR',         ext: 'mov', codec: 'dnxhr'                   },
      { id: 'codec-cineform',  label: 'CineForm',      ext: 'mov', codec: 'cineform'                },
      { id: 'codec-qtanim',    label: 'QT Animation',  ext: 'mov', codec: 'qtrle'                   },
      { id: 'codec-uncomp',    label: 'Uncompressed',  ext: 'mov', codec: 'rawvideo'                },
      { id: 'codec-ffv1',      label: 'FFV1',          ext: 'mkv', codec: 'ffv1'                    },
      { id: 'codec-xdcam422',  label: 'XDCAM HD422',   ext: 'mov', codec: 'xdcam422'               },
      { id: 'codec-xdcam35',   label: 'XDCAM HD35',    ext: 'mov', codec: 'xdcam35'                },
      { id: 'codec-avcintra',  label: 'AVC-Intra',     ext: 'mov', codec: 'h264',       todo: true },
      { id: 'codec-xavc',      label: 'XAVC',          ext: 'mp4', codec: 'h264',       todo: true },
      { id: 'codec-xavclgop',  label: 'XAVC Long GOP', ext: 'mp4', codec: 'h264',       todo: true },
      { id: 'codec-hap',       label: 'HAP',           ext: 'mov', codec: 'hap'                    },
      { id: 'codec-theora',    label: 'Theora',        ext: 'ogv', codec: 'theora'                  },
      { id: 'codec-mpeg2',     label: 'MPEG-2',        ext: 'mpg', codec: 'mpeg2video'              },
      { id: 'codec-mjpeg',     label: 'MJPEG',         ext: 'mov', codec: 'mjpeg'                   },
      { id: 'codec-xvid',      label: 'Xvid',          ext: 'avi', codec: 'mpeg4'                   },
      { id: 'codec-dv',        label: 'DV',            ext: 'mov', codec: 'dvvideo'                 },
      { id: 'codec-mpeg1',     label: 'MPEG-1',        ext: 'mpg', codec: 'mpeg1video'              },
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
    { label: '3D Model', cat: 'model', fmts: [
      { id: 'obj' }, { id: 'gltf' }, { id: 'glb' },
      { id: 'stl' }, { id: 'ply' }, { id: 'dae', label: 'COLLADA' },
      { id: '3ds' }, { id: 'x3d' },
      // FBX write is ASCII-only via assimp (binary FBX needs Autodesk SDK).
      { id: 'fbx', label: 'FBX (ASCII)' },
      // Pro animation / CAD — placeholder scaffolding.
      { id: 'usd',   label: 'USD',       preview: true },
      { id: 'usdz',  label: 'USDZ',      preview: true },
      { id: 'abc',   label: 'Alembic',   preview: true },
      { id: 'blend', label: 'Blender',   preview: true },
      { id: 'step',  label: 'STEP',      todo: true, preview: true },
      { id: 'iges',  label: 'IGES',      todo: true, preview: true },
    ]},
    // Intact Video Operations — mechanical operations that stream-copy the
    // video track (no re-encode). Fast, lossless, metadata/container-level.
    // Subtitling lives here because soft-subs don't touch the video; the
    // operation page itself exposes soft vs hard as a mode toggle and is
    // also surfaced from Analysis, both routes land on the same page.
    { label: 'Intact Video Operations', cat: 'intact', fmts: [
      { id: 'cut-noenc', label: 'Cut/Extract' },
      { id: 'replace-audio', label: 'Replace Audio' },
      { id: 'rewrap', label: 'Rewrap' },
      { id: 'merge', label: 'Merge' },
      { id: 'extract', label: 'Extract' },
      { id: 'subtitling', label: 'Subtitling' },
      { id: 'remove-audio', label: 'Remove Audio', ops: true },
      { id: 'remove-video', label: 'Remove Video', ops: true },
      { id: 'metadata-strip', label: 'Strip Metadata', ops: true },
      { id: 'loop', label: 'Loop', ops: true },
    ]},
    // Processing — operations that must re-encode the video track (filter
    // chain, fps/res change, pixel alteration, timeline mutation).
    { label: 'Processing', cat: 'processing', fmts: [
      { id: 'conform', label: 'Conform' },
      { id: 'silence-remove', label: 'Silence Remover' },
      { id: 'video-inserts', label: 'Video Inserts', todo: true },
      { id: 'rotate-flip', label: 'Rotate / Flip', ops: true },
      { id: 'speed', label: 'Speed / Reverse', ops: true },
      { id: 'fade', label: 'Crop / Fade', ops: true },
      { id: 'deinterlace', label: 'Deinterlace', ops: true },
      { id: 'denoise', label: 'Denoise', ops: true },
      { id: 'thumbnail', label: 'Thumbnail', ops: true },
      { id: 'contact-sheet', label: 'Contact Sheet', ops: true },
      { id: 'frame-export', label: 'Frame Export', ops: true },
      { id: 'watermark', label: 'Watermark', ops: true },
      { id: 'channel-tools', label: 'Channel Tools', ops: true },
      { id: 'pad-silence', label: 'Pad Silence', ops: true },
    ]},
    // Chroma Key — three tiers of background removal / keying.
    //   chroma-ffmpeg : built-in, ffmpeg `chromakey` / `colorkey` filter, clean shots only.
    //   chroma-rvm    : bundled Robust Video Matting (MIT, ~15 MB ONNX), neural matting, no green screen needed.
    //   chroma-corridor: managed install of CorridorKey (non-commercial, ~3 GB), best for hair/motion-blur on green-screen shots.
    { label: 'Chroma Key', cat: 'chroma', fmts: [
      { id: 'chroma-ffmpeg',   label: 'Chroma Key (FFmpeg)', ops: true },
      { id: 'chroma-rvm',      label: 'Neural Matte (RVM)',  todo: true },
      { id: 'chroma-corridor', label: 'CorridorKey',         todo: true },
    ]},
    { label: 'AI Tools', cat: 'ai', fmts: [
      { id: 'ai-sep', label: 'Audio Separation', todo: true },
      { id: 'ai-transcribe', label: 'Transcription', todo: true },
      { id: 'ai-translate', label: 'Translate', todo: true },
      { id: 'ai-colorize', label: 'Colorize', todo: true },
      { id: 'ai-bgremove', label: 'BG Remover', todo: true },
      // Cross-category shortcut — same target page as Intact → Subtitling.
      // `ops: true` marks this as an operation-launching entry even though
      // it lives in a non-ops category.
      { id: 'subtitling', label: 'Subtitling', todo: true, ops: true },
    ]},
    { label: 'Analysis', cat: 'analysis', fmts: [
      // ops:true routes the click into operations mode instead of setting it as
      // an output format — analysis tools produce reports, not output files.
      { id: 'loudness', label: 'Loudness & TP', ops: true },
      { id: 'audio-norm', label: 'Audio Norm', ops: true },
      { id: 'cut-detect', label: 'Cut Detection', ops: true },
      { id: 'black-detect', label: 'Black Detection', ops: true },
      { id: 'vmaf', label: 'VMAF', ops: true },
      { id: 'framemd5', label: 'FrameMD5', ops: true },
      // Same unified Subtitling page — surfaces from Analysis so users who
      // look for "subtitle lint / diff / detect" find it where they expect.
      { id: 'subtitling', label: 'Subtitling', ops: true },
    ]},
    { label: 'Burn & Rip', cat: 'burn', fmts: [
      { id: 'dvd', label: 'DVD', todo: true },
      { id: 'bluray', label: 'Blu-ray', todo: true },
      { id: 'dvd-rip', label: 'DVD Rip', todo: true },
      { id: 'web-video', label: 'Web Video', todo: true },
    ]},
    // "Files" super-section (rendered at the bottom of the right sidebar as
    // its own collapsible super-section, alongside Conversion and Tools).
    // The three groups below stay as independent subcategories with their
    // own `cat` so all downstream routing (options objects, compat filter)
    // keeps working unchanged.
    { label: 'Data', cat: 'data', fmts: [
      { id: 'json' }, { id: 'csv' }, { id: 'tsv' }, { id: 'xml' }, { id: 'yaml' },
      // Data-nerd: sqlite via rusqlite (bundled), parquet via duckdb CLI,
      // jupyter via `jupyter nbconvert`. See src-tauri/src/convert/data.rs
      // and src-tauri/src/convert/notebook.rs. These are INPUT format
      // markers — target codec flows through the standard data pipeline.
      { id: 'sqlite',  label: 'SQLite'  },
      { id: 'parquet', label: 'Parquet' },
      { id: 'ipynb',   label: 'Jupyter' },
    ]},
    { label: 'Document', cat: 'document', fmts: [
      { id: 'html' }, { id: 'pdf' }, { id: 'txt' }, { id: 'md' },
    ]},
    // Office — LibreOffice managed-install lane. `.pptx/.docx/.xlsx/.key/
    // .pages/.numbers` are all ZIP containers, so Media Extract (pull all
    // embedded images/videos/audio out to a folder) can ship without the
    // LibreOffice install. Full format conversion (→ PDF, images, HTML,
    // MP4 slideshow) needs LibreOffice headless (~300 MB install).
    { label: 'Office', cat: 'office', fmts: [
      { id: 'pptx',    label: 'PPTX',    todo: true, preview: true },
      { id: 'ppt',     label: 'PPT',     todo: true, preview: true },
      { id: 'docx',    label: 'DOCX',    todo: true, preview: true },
      { id: 'doc',     label: 'DOC',     todo: true, preview: true },
      { id: 'xlsx',    label: 'XLSX',    todo: true, preview: true },
      { id: 'xls',     label: 'XLS',     todo: true, preview: true },
      { id: 'odt',     label: 'ODT',     todo: true, preview: true },
      { id: 'odp',     label: 'ODP',     todo: true, preview: true },
      { id: 'ods',     label: 'ODS',     todo: true, preview: true },
      { id: 'rtf',     label: 'RTF',     todo: true, preview: true },
      { id: 'key',     label: 'Keynote', todo: true, preview: true },
      { id: 'pages',   label: 'Pages',   todo: true, preview: true },
      { id: 'numbers', label: 'Numbers', todo: true, preview: true },
    ]},
    // Ebooks — Calibre managed install provides ebook-convert across all
    // these formats plus PDF/EPUB conversion.
    { label: 'Ebook', cat: 'ebook', todo: true, fmts: [
      { id: 'epub',  label: 'EPUB' },
      { id: 'mobi',  label: 'MOBI' },
      { id: 'azw3',  label: 'AZW3' },
      { id: 'fb2',   label: 'FB2'  },
      { id: 'lit',   label: 'LIT'  },
    ]},
    // Subtitle file-format conversion (distinct from the Subtitling
    // operation — this is SRT-to-VTT style plumbing, not video embed/burn).
    { label: 'Subtitle', cat: 'subtitle', fmts: [
      { id: 'srt',  label: 'SRT'  },
      { id: 'vtt',  label: 'VTT'  },
      { id: 'ass',  label: 'ASS'  },
      { id: 'ssa',  label: 'SSA'  },
      // SBV (YouTube) is hand-rolled via a SRT bridge; TTML write is
      // ffmpeg-native. Both now ship.
      { id: 'sbv',  label: 'SBV'  },
      { id: 'ttml', label: 'TTML' },
    ]},
    // Timeline / edit decision lists — pro workflow interop (Premiere,
    // Resolve, FCP, Avid). Routed through OpenTimelineIO's `otioconvert`
    // CLI (the Rosetta stone format). `.xml` (Premiere XML) is routed to
    // the timeline pipeline when either side of the conversion is a known
    // timeline-native extension (edl/fcpxml/otio/aaf); generic data XML
    // still falls through to the data pipeline.
    { label: 'Timeline', cat: 'timeline', fmts: [
      { id: 'edl',    label: 'EDL' },
      { id: 'fcpxml', label: 'FCPXML' },
      { id: 'xml',    label: 'Premiere XML' },
      { id: 'otio',   label: 'OTIO' },
      { id: 'aaf',    label: 'AAF' },
    ]},
    { label: 'Archive', cat: 'archive', fmts: [
      { id: 'zip' }, { id: 'tar' }, { id: 'gz' }, { id: '7z' },
      { id: 'iso', label: 'ISO' },
      { id: 'dmg', label: 'DMG' },
      { id: 'cbr', label: 'CBR' },
      { id: 'cbz', label: 'CBZ' },
      { id: 'rar', label: 'RAR' },
    ]},
    // Font containers — routed through fonttools (Python). Flavor swap only:
    // ttf↔otf does not re-encode outlines (glyf vs. CFF is preserved).
    { label: 'Font', cat: 'font', fmts: [
      { id: 'ttf',   label: 'TTF' },
      { id: 'otf',   label: 'OTF' },
      { id: 'woff',  label: 'WOFF' },
      { id: 'woff2', label: 'WOFF2' },
    ]},
    { label: 'Email', cat: 'email', todo: true, fmts: [
      // MSG (Outlook binary) remains deferred — backend returns a clear error.
      { id: 'msg',  label: 'MSG',  todo: true, preview: true },
      { id: 'eml',  label: 'EML'  },
      { id: 'mbox', label: 'MBOX' },
    ]},
  ];

  function categoryFor(fmt) {
    if (!fmt) return null;
    for (const g of FORMAT_GROUPS) {
      const hit = g.fmts.find(f => f.id === fmt);
      if (hit) return hit.cat ?? g.cat;
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
    model:    ['model'],
    timeline: ['timeline'],
    font:     ['font'],
    subtitle: ['subtitle'],
    ebook:    ['ebook'],
    email:    ['email'],
  };
  let compatibleOutputCats = $derived.by(() => {
    if (selectedIds.size > 1) {
      // Union of all selected items' compatible categories so the picker shows
      // everything any selected file can be converted to.
      const cats = new Set();
      for (const id of selectedIds) {
        const item = queue.find(q => q.id === id);
        const itemCats = item ? (OUTPUT_CATS_FOR[item.mediaType] ?? []) : [];
        for (const c of itemCats) cats.add(c);
      }
      return cats.size > 0 ? [...cats] : null;
    }
    return selectedItem ? (OUTPUT_CATS_FOR[selectedItem.mediaType] ?? null) : null;
  });

  // FORMAT_GROUPS sorted so compatible categories float to the top whenever
  // a file is selected — prevents useful options falling below the scroll
  // fold. Reverts to default order as soon as nothing is selected.
  let sortedFormatGroups = $derived.by(() => {
    if (!compatibleOutputCats) return FORMAT_GROUPS;
    const groupCats = (g) => {
      const s = new Set(g.fmts.map(f => f.cat ?? g.cat));
      return s;
    };
    return [...FORMAT_GROUPS].sort((a, b) => {
      const aOk = [...groupCats(a)].some(c => compatibleOutputCats.includes(c));
      const bOk = [...groupCats(b)].some(c => compatibleOutputCats.includes(c));
      return aOk === bOk ? 0 : (aOk ? -1 : 1);
    });
  });

  let activeOutputCategory = $derived(categoryFor(globalOutputFormat));

  // Per-operation media-type compatibility — gates the file queue when in ops mode.
  const OP_COMPAT = {
    'cut-noenc':     ['video', 'audio'],
    'replace-audio': ['video'],
    'rewrap':        ['video', 'audio'],
    'conform':       ['video'],
    'merge':         ['video', 'audio'],
    'extract':       ['video'],
    'subtitling':    ['video'],
    'video-inserts': ['video'],
    'silence-remove': ['video', 'audio'],
    // Intact stream-copy tools
    'remove-audio':   ['video'],
    'remove-video':   ['video'],
    'metadata-strip': ['video', 'audio'],
    'loop':           ['video', 'audio'],
    // Video processing filters
    'rotate-flip':    ['video'],
    'speed':          ['video', 'audio'],
    'fade':           ['video', 'audio'],
    'deinterlace':    ['video'],
    'denoise':        ['video'],
    // Frame / image tools
    'thumbnail':      ['video'],
    'contact-sheet':  ['video'],
    'frame-export':   ['video'],
    'watermark':      ['video'],
    // Audio processing
    'channel-tools':  ['video', 'audio'],
    'pad-silence':    ['video', 'audio'],
    // Chroma key
    'chroma-ffmpeg':  ['video'],
    // Analysis ops
    'loudness':      ['video', 'audio'],   // needs an audio track
    'audio-norm':    ['video', 'audio'],
    'cut-detect':    ['video'],
    'black-detect': ['video'],
    'vmaf':          ['video'],            // reference & distorted both video
    'framemd5':      ['video', 'audio'],   // per-frame hash works on either
  };
  let compatibleTypes = $derived(
    // Operations mode overrides output-category compat
    operationsMode && selectedOperation ? (OP_COMPAT[selectedOperation] ?? []) :
    !activeOutputCategory ? [] :
    activeOutputCategory === 'audio' ? ['audio', 'video'] :
    activeOutputCategory === 'video' ? ['video'] :
    activeOutputCategory === 'codec' ? ['video'] :
    activeOutputCategory === 'image' ? ['image'] :
    activeOutputCategory === 'data'  ? ['data'] :
    activeOutputCategory === 'document' ? ['document'] :
    activeOutputCategory === 'archive'  ? ['archive'] :
    activeOutputCategory === 'model'    ? ['model'] :
    activeOutputCategory === 'timeline' ? ['timeline'] :
    activeOutputCategory === 'font'     ? ['font'] :
    activeOutputCategory === 'subtitle' ? ['subtitle'] :
    activeOutputCategory === 'ebook'    ? ['ebook'] :
    activeOutputCategory === 'email'    ? ['email'] :
    []
  );

  // If the currently-selected item becomes incompatible with a newly-chosen
  // output format, clear the selection so the user doesn't end up acting on
  // an item they're not allowed to select.
  $effect(() => {
    if (!selectedItem) return;
    if (compatibleTypes.length > 0 && !compatibleTypes.includes(selectedItem.mediaType)) {
      queueManagerEl?.handleSelect(null);
    }
  });

  // handleSelect / deselectAll / _selectableIds live in QueueManager.
  // App.svelte callers delegate via queueManagerEl.

  function deselectAll() { queueManagerEl?.deselectAll(); }

</script>

<svelte:window
  onmousemove={(e) => { onDiffWindowMouseMove(e); }}
  onmouseup={() => { onDiffWindowMouseUp(); }}
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

<!-- Hidden single-file input shared by auxiliary ops pickers (VMAF ref,
     subtitle diff, frame-md5 diff, etc.). Target setter is held in JS
     state between click and change. -->
<input
  type="file"
  bind:this={auxFileInput}
  onchange={onAuxFileChange}
  class="hidden"
  aria-hidden="true"
/>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="relative flex h-full bg-[var(--surface)] overflow-hidden select-none"
     ondragover={onWindowDragover}
     ondragleave={onWindowDragleave}
     ondrop={onWindowDrop}>

  <!-- ── 3-column body (full height, no titlebar) ───────────────────────────── -->

    <!-- ── LEFT: File queue (320px expanded / 229px compact) ──────────────── -->
    <aside class="{queueCompact ? 'w-[229px]' : 'w-[320px]'} shrink-0 border-r border-[var(--border)] flex flex-col bg-[var(--surface-raised)] relative {settingsOpen ? 'z-[500]' : 'z-50'}"
           role="region" aria-label="File queue">

      <!-- Queue header — pl-20 clears macOS traffic lights.
           Matches the right-sidebar header exactly (subtle accent wash,
           blue-tinted bottom border, py-3 + larger buttons) so the two
           "control planes" read as a unified band at the top of the app.
           shrink-0 + outer aside's flex-col means the list below scrolls
           underneath it. -->
      <div class="flex items-center justify-between gap-1.5 pl-[64px] pr-2 py-1.5 shrink-0"
           data-tauri-drag-region
           style="background:color-mix(in srgb, var(--accent) 6%, var(--surface-raised));
                  border-bottom:1px solid color-mix(in srgb, var(--accent) 45%, var(--border))">
        <!-- Left: Proxy Node (flipped from right) -->
        <button
          onclick={() => queueManagerEl?.addBatchFolder()}
          title="Add a proxy node — batch files together for matched rename / output routing"
          class="btn-bevel flex items-center gap-1 px-2 py-0.5 text-[11px] shrink-0"
        >
          <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor"
               stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <path d="M21 16V8a2 2 0 0 0-1-1.73l-7-4a2 2 0 0 0-2 0l-7 4A2 2 0 0 0 3 8v8a2 2 0 0 0 1 1.73l7 4a2 2 0 0 0 2 0l7-4A2 2 0 0 0 21 16z"/>
            <polyline points="3.27 6.96 12 12.01 20.73 6.96"/>
            <line x1="12" y1="22.08" x2="12" y2="12"/>
          </svg>
          Proxy Node
        </button>
        <!-- Right: Compact / Expanded segmented (flipped from left) -->
        <div class="btn-segmented flex items-stretch shrink-0">
          <button
            onclick={() => queueCompact = true}
            title="Compact list"
            class="btn-bevel btn-seg w-9 py-1 px-2 flex items-center justify-center {queueCompact ? 'is-active' : 'is-muted'}"
          >
            <svg width="13" height="13" viewBox="0 0 13 13" fill="currentColor">
              <rect y="0"    width="13" height="2" rx="0.5"/>
              <rect y="3.67" width="13" height="2" rx="0.5"/>
              <rect y="7.33" width="13" height="2" rx="0.5"/>
              <rect y="11"   width="13" height="2" rx="0.5"/>
            </svg>
          </button>
          <button
            onclick={() => queueCompact = false}
            title="Expanded list"
            class="btn-bevel btn-seg w-8 flex items-center justify-center {!queueCompact ? 'is-active' : 'is-muted'}"
          >
            <svg width="13" height="11" viewBox="0 0 13 11" fill="currentColor">
              <rect y="0"   width="13" height="3" rx="0.75"/>
              <rect y="8"   width="13" height="3" rx="0.75"/>
            </svg>
          </button>
        </div>
      </div>

      <!-- Search field — filters the queue by filename / extension -->
      <div class="shrink-0 px-2 py-1.5 border-b border-[var(--border)]"
           style="background:color-mix(in srgb, var(--surface-raised) 60%, #000 40%)">
        <div class="relative">
          <svg class="absolute left-2 top-1/2 -translate-y-1/2 text-[var(--text-secondary)] pointer-events-none"
               width="11" height="11" viewBox="0 0 16 16" fill="none" stroke="currentColor"
               stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round">
            <circle cx="7" cy="7" r="5"/>
            <path d="M14 14l-3.5-3.5"/>
          </svg>
          <input
            type="text"
            bind:value={leftSearch}
            placeholder=""
            class="w-full pl-7 pr-6 py-1 text-[11px] rounded border border-[var(--border)]
                   bg-[color:color-mix(in_srgb,#000_40%,var(--surface-raised))]
                   text-[var(--text-primary)] placeholder:text-[var(--text-secondary)]
                   focus:outline-none focus:border-[var(--accent)]"
          />
          {#if leftSearch}
            <button
              onclick={() => leftSearch = ''}
              title="Clear filter"
              class="absolute right-1.5 top-1/2 -translate-y-1/2 text-[var(--text-secondary)] hover:text-[var(--text-primary)] text-[13px] leading-none"
            >×</button>
          {/if}
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

      <!-- File list — rendered by QueueManager which owns queue state, selection,
           drag handlers, and the async preview pipeline (runLoadPipeline / _loadGen) -->
      <QueueManager
        bind:this={queueManagerEl}
        bind:queue
        bind:selectedItem
        bind:selectedMediaType
        bind:selectedIds
        bind:draggingFileId
        bind:liveSrc
        bind:selectedDuration
        bind:selectedWidth
        bind:selectedHeight
        bind:previewLoading
        bind:tlMediaReady
        bind:tlWaveformReady
        bind:tlSpectrogramReady
        bind:tlFilmstripReady
        bind:cachedWaveformForTimeline
        bind:cachedFilmstripForTimeline
        visibleQueue={visibleQueue}
        {setStatus}
        {compatibleTypes}
        {onSelectionChange}
        compact={queueCompact}
        showExtColumn={settings.fileTypeColumn}
        brightFiletype={settings.brightFiletype}
        {itemHasEdits}
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

      <!-- ── Queue-action bar: Clear / Add / Deselect — lives in its own
           gray panel above the black control panel, visually separated by
           a single hairline. Center-justified. ───────────────────────── -->
      <div class="shrink-0 border-t border-[var(--border)] flex items-center gap-1.5 px-3 py-1.5"
           style="background:var(--surface-raised)">
        <button
          onclick={clearQueue}
          disabled={queue.length === 0}
          class="btn-bevel flex-1 py-0.5 text-[11px]"
        >Clear</button>
        <button
          onclick={onBrowse}
          class="btn-bevel flex-1 py-0.5 text-[11px]"
        >Add Files</button>
        <button
          onclick={deselectAll}
          disabled={selectedIds.size === 0}
          class="btn-bevel flex-1 py-0.5 text-[11px]"
        >Deselect</button>
      </div>

      <!-- ── Bottom panel: output + convert + settings ──────────────────────── -->
      <div class="shrink-0 border-t border-[var(--border)] flex flex-col gap-2 px-3 py-2.5"
           style="background:color-mix(in srgb, var(--surface-raised) 60%, #000 40%)">

        <!-- Progress — current job on top, queue completion below.
             Label column is a fixed width so both bars start at the same x. -->
        <div class="flex flex-col gap-1.5">
          <div class="flex items-center gap-2">
            <span class="w-12 text-right text-[9px] font-semibold uppercase tracking-wider text-[var(--text-secondary)]">Current</span>
            <div class="flex-1"><ProgressBar value={currentPercent} label="Current job" /></div>
          </div>
          <div class="flex items-center gap-2">
            <span class="w-12 text-right text-[9px] font-semibold uppercase tracking-wider text-[var(--text-secondary)]">All</span>
            <div class="flex-1"><ProgressBar value={overallPercent} label="Queue" /></div>
          </div>
        </div>

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
          {#if operationsMode}
            <div class="flex items-center gap-2 pointer-events-none opacity-40">
              <span class="w-12 text-right text-[9px] font-semibold uppercase tracking-wider text-[var(--text-secondary)] shrink-0">Convert</span>
              <div class="flex-1 flex gap-1.5">
                <div class="flex-1 py-1 rounded text-[11px] font-medium text-center border border-red-900/60 text-red-700">Selected</div>
                <div class="flex-1 py-1 rounded text-[11px] font-semibold text-center bg-red-950/50 text-red-700">All</div>
              </div>
            </div>
          {:else}
            <div class="flex items-center gap-2">
              <span class="w-12 text-right text-[9px] font-semibold uppercase tracking-wider text-[var(--text-secondary)] shrink-0">Convert</span>
              <div class="flex-1 flex gap-1.5">
                <button
                  onclick={() => startConvert('selected')}
                  disabled={!selectedItem || queue.length === 0 || !globalOutputFormat}
                  class="flex-1 py-1 rounded text-[11px] font-medium transition-colors border
                         {!selectedItem || queue.length === 0 || !globalOutputFormat
                           ? 'border-[var(--border)] text-[var(--text-secondary)] cursor-not-allowed opacity-40'
                           : 'border-[var(--border)] text-[var(--text-primary)] hover:bg-[var(--accent)] hover:text-white hover:border-[color-mix(in_srgb,var(--accent)_70%,#000)]'}"
                >Selected</button>
                <button
                  onclick={() => startConvert('all')}
                  disabled={queue.length === 0 || !globalOutputFormat}
                  class="flex-1 py-1 rounded text-[11px] font-semibold transition-colors
                         {queue.length === 0 || !globalOutputFormat
                           ? 'bg-[var(--border)] text-[var(--text-secondary)] cursor-not-allowed opacity-40'
                           : 'bg-[var(--accent)] text-white hover:opacity-90'}"
                >All</button>
              </div>
            </div>
          {/if}
        {/if}

        <!-- Quick output controls — exposed here because Source/Custom dest
             and the suffix get tweaked far more often than anything else in
             Settings. Mirrors the same bindings; the Settings panel still has
             the canonical copy. -->
        <div class="flex flex-col gap-1.5">
          <div class="flex items-stretch gap-1">
            <button
              onclick={() => outputDestMode = 'source'}
              title="Write outputs alongside the source files"
              class="flex items-center justify-center gap-1.5 px-2 py-1 rounded border transition-colors flex-1 min-w-0 text-[11px]
                     {outputDestMode === 'source'
                       ? 'bg-[var(--accent)] text-white border-[color-mix(in_srgb,var(--accent)_70%,#000)]'
                       : 'border-[var(--border)] text-[var(--text-secondary)] hover:bg-[var(--accent)] hover:text-white hover:border-[color-mix(in_srgb,var(--accent)_70%,#000)]'}"
            >
              <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" class="shrink-0">
                <path d="M3 9l9-7 9 7v11a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z"/>
                <polyline points="9 22 9 12 15 12 15 22"/>
              </svg>
              {#if !queueCompact}Source{/if}
            </button>
            <button
              onclick={() => { outputDestMode = 'custom'; folderInput?.click(); }}
              title={customOutputDir ? `Output → ${customOutputDir}` : 'Pick an output folder'}
              class="flex items-center justify-center gap-1.5 px-2 py-1 rounded border transition-colors flex-1 min-w-0 text-[11px]
                     {outputDestMode === 'custom'
                       ? 'bg-[var(--accent)] text-white border-[color-mix(in_srgb,var(--accent)_70%,#000)]'
                       : 'border-[var(--border)] text-[var(--text-secondary)] hover:bg-[var(--accent)] hover:text-white hover:border-[color-mix(in_srgb,var(--accent)_70%,#000)]'}"
            >
              <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" class="shrink-0">
                <path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z"/>
                <line x1="12" y1="11" x2="12" y2="17"/>
                <polyline points="9 14 12 17 15 14"/>
              </svg>
              {#if !queueCompact}Browse{/if}
            </button>
            <input
              type="text"
              bind:value={outputSeparator}
              maxlength="1"
              disabled={converting}
              onfocus={(e) => e.currentTarget.select()}
              title="Single character placed between filename and suffix (default: _)"
              class="w-[24px] px-1 py-1 text-[11px] text-center rounded border border-[var(--border)]
                     bg-[var(--surface)] text-[var(--text-primary)] outline-none font-mono
                     focus:border-[var(--accent)] transition-colors disabled:opacity-40"
            />
            <input
              type="text"
              bind:value={outputSuffix}
              disabled={converting}
              placeholder="suffix"
              onfocus={(e) => e.currentTarget.select()}
              title="Suffix appended to output filenames (e.g. name{outputSeparator}{outputSuffix}.mp4)"
              class="w-[88px] px-2 py-1 text-[11px] rounded border border-[var(--border)]
                     bg-[var(--surface)] text-[var(--text-primary)] outline-none font-mono
                     focus:border-[var(--accent)] transition-colors disabled:opacity-40"
            />
          </div>
        </div>

        <!-- Settings button (snug) + status box (fills remaining width, right-aligned) -->
        <div class="flex items-stretch gap-1.5 relative">
          {#if settingsOpen}
            <!-- Floating settings panel — fixed width, anchored to the right
                 edge of the settings button so compact sidebar doesn't squish it.
                 Overflows into the preview area; the backdrop dims everything. -->
            <!-- svelte-ignore a11y_no_static_element_interactions -->
            <div
              style="position:absolute; bottom:100%; left:-0.75rem; width:360px; max-height:70vh;
                     background:color-mix(in srgb, var(--surface-raised) 96%, #000 4%);
                     border:1px solid var(--border);
                     border-radius:10px;
                     box-shadow:0 -10px 36px rgba(0,0,0,0.6);
                     margin-bottom:0.5rem;
                     overflow-y:auto; z-index:500"
              onmousedown={(e) => e.stopPropagation()}
            >
              <UpdateManager
                bind:this={updateManagerEl}
                bind:updateState
                {settings}
                {setStatus}
                onUploadBeforeUpdate={_uploadBeforeUpdate}
              />

              <!-- Section: Diagnostics ──────────────────────────────────────
                   Lists errors captured this and prior sessions (updater
                   failures, job errors, uncaught JS). "Copy" puts a plain
                   text report on the clipboard for a bug report.
                   Network: the uploader only fires when the user clicks
                   Update Now / Download update — no background traffic,
                   ever. During beta the toggle is forced on with a red
                   warning; after 1.0 it becomes a normal opt-in. -->
              <div class="px-4 pt-3 pb-3 border-b border-[var(--border)] flex flex-col gap-2">
                <div class="flex items-center gap-3">
                  <p class="text-[10px] font-semibold uppercase tracking-widest text-[var(--text-secondary)] shrink-0">Diagnostics</p>
                  <span class="text-[11px] {diagEntries.length > 0 ? 'text-red-400' : 'text-[var(--text-secondary)]'}">
                    {diagEntries.length === 0
                      ? 'No errors this session'
                      : (diagEntries.length === 1 ? '1 error this session' : `${diagEntries.length} errors this session`)}
                  </span>
                  {#if diagEntries.length > 0}
                    <button onclick={() => diagnosticsExpanded = !diagnosticsExpanded}
                            class="ml-auto text-[11px] text-[var(--text-secondary)] hover:text-[var(--text-primary)] transition-colors shrink-0">
                      {diagnosticsExpanded ? 'Hide' : 'Show'}
                    </button>
                  {/if}
                </div>
                {#if diagnosticsExpanded && diagEntries.length > 0}
                  <div class="max-h-[140px] overflow-y-auto rounded border border-[var(--border)]
                              bg-[var(--surface)] font-mono text-[10px] leading-snug p-2 flex flex-col gap-1">
                    {#each diagEntries.slice().reverse() as e (e.t + e.source + e.message)}
                      <div>
                        <span class="text-[var(--text-secondary)]">{new Date(e.t).toLocaleTimeString()}</span>
                        <span class="text-[var(--accent)]">[{e.source}]</span>
                        <span class="text-[var(--text-primary)]">{e.message}</span>
                      </div>
                    {/each}
                  </div>
                {/if}
                <div class="flex items-center gap-1.5">
                  <button onclick={copyDiagnostics}
                          class="px-2.5 py-1 rounded text-[11px] border border-[var(--border)]
                                 text-[var(--text-secondary)] hover:text-[var(--text-primary)]
                                 hover:border-[var(--accent)] transition-colors">
                    Copy diagnostics
                  </button>
                  {#if diagEntries.length > 0}
                    <button onclick={clearDiagnostics}
                            class="px-2.5 py-1 rounded text-[11px] border border-[var(--border)]
                                   text-[var(--text-secondary)] hover:text-[var(--text-primary)]
                                   hover:border-[var(--accent)] transition-colors">
                      Clear
                    </button>
                  {/if}
                  {#if _diagCopyNote}
                    <span class="text-[11px] text-[var(--text-secondary)] ml-1">{_diagCopyNote}</span>
                  {/if}
                </div>

                <!-- Opt-in uploader — piggybacks on update clicks. During
                     beta the checkbox is forced on with a red frame and
                     explanatory copy so there's no surprise. -->
                <label class="flex items-start gap-2 mt-1 {BETA ? 'p-1.5 rounded border border-red-600/70 bg-red-900/10' : ''}">
                  <input type="checkbox"
                         checked={BETA || settings.sendDiagnostics}
                         disabled={BETA}
                         onchange={(e) => { if (!BETA) settings.sendDiagnostics = e.currentTarget.checked; }}
                         class="w-3.5 h-3.5 accent-[var(--accent)] mt-0.5" />
                  <span class="text-[11px] leading-snug {BETA ? 'text-red-200' : 'text-[var(--text-primary)]'}">
                    {#if BETA}
                      <strong>Beta build:</strong> diagnostics are submitted when you install an update.
                      Required during beta testing. Contents are visible above — click "Copy diagnostics" to preview.
                    {:else}
                      Submit diagnostics when I install an update. Nothing is sent in the background — only
                      when you click Update Now.
                    {/if}
                  </span>
                </label>
              </div>

              <!-- Section: Developer -->
              <div class="px-4 pt-3 pb-3 border-b border-[var(--border)] flex flex-col gap-2.5">
                <p class="text-[10px] font-semibold uppercase tracking-widest text-[var(--text-secondary)]">Developer</p>
                <label class="flex items-start gap-2 p-1.5 rounded border border-red-600/70 bg-red-900/10">
                  <input type="checkbox"
                         bind:checked={settings.showDevFeatures}
                         class="w-3.5 h-3.5 accent-[var(--accent)] mt-0.5" />
                  <span class="text-[11px] leading-snug text-red-200">
                    <strong>Show developer features</strong> — displays unfinished formats and tools (green items) in the format picker. Uncheck to preview what ships to users.
                  </span>
                </label>
              </div>

              <!-- Section: Output -->
              <div class="px-4 pt-3 pb-3 border-b border-[var(--border)] flex flex-col gap-2.5">
                <p class="text-[10px] font-semibold uppercase tracking-widest text-[var(--text-secondary)]">Output</p>
                <!-- Destination -->
                <div class="flex flex-col gap-1.5">
                  <div class="flex gap-1.5">
                    <button
                      onclick={() => outputDestMode = 'source'}
                      class="flex items-center gap-1.5 px-2 py-1 rounded text-[11px] border transition-colors flex-1
                             {outputDestMode === 'source'
                               ? 'border-[var(--accent)] text-[var(--accent)] bg-[var(--accent)]/10'
                               : 'border-[var(--border)] text-[var(--text-secondary)] hover:bg-[var(--accent)] hover:text-white hover:border-[color-mix(in_srgb,var(--accent)_70%,#000)]'}"
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
                               : 'border-[var(--border)] text-[var(--text-secondary)] hover:bg-[var(--accent)] hover:text-white hover:border-[color-mix(in_srgb,var(--accent)_70%,#000)]'}"
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
                               text-[var(--text-secondary)] hover:bg-[var(--accent)] hover:text-white hover:border-[color-mix(in_srgb,var(--accent)_70%,#000)] transition-colors shrink-0"
                      >Browse</button>
                    </div>
                  {/if}
                </div>
                <!-- Suffix -->
                <div class="flex items-center gap-2">
                  <label for="output-suffix" class="text-[11px] text-[var(--text-secondary)] whitespace-nowrap shrink-0">Suffix</label>
                  <input
                    type="text"
                    bind:value={outputSeparator}
                    maxlength="1"
                    disabled={converting}
                    onfocus={(e) => e.currentTarget.select()}
                    title="Separator character (default: _)"
                    class="w-[28px] px-1 py-1 text-[12px] text-center rounded border border-[var(--border)]
                           bg-[var(--surface)] text-[var(--text-primary)] outline-none
                           focus:border-[var(--accent)] transition-colors disabled:opacity-40 font-mono"
                  />
                  <input
                    id="output-suffix"
                    type="text"
                    bind:value={outputSuffix}
                    disabled={converting}
                    placeholder="converted"
                    onfocus={(e) => e.currentTarget.select()}
                    class="flex-1 min-w-0 px-2 py-1 text-[12px] rounded border border-[var(--border)]
                           bg-[var(--surface)] text-[var(--text-primary)] outline-none
                           focus:border-[var(--accent)] transition-colors disabled:opacity-40 font-mono"
                  />
                </div>
              </div>

              <!-- Section: UI -->
              <div class="px-4 pt-3 pb-3 border-b border-[var(--border)] flex flex-col gap-2.5">
                <p class="text-[10px] font-semibold uppercase tracking-widest text-[var(--text-secondary)]">UI</p>
                <!-- Visualizer default -->
                <div class="flex items-center justify-between gap-2">
                  <span class="text-[12px] text-[var(--text-primary)]">Visualizer Auto Expand</span>
                  <div class="flex rounded overflow-hidden border border-[var(--border)]">
                    {#each [['no','Off'],['audio','Audio'],['av','A+V']] as [val, label]}
                      <button
                        onclick={() => settings.vizDefault = val}
                        class="px-2 py-1 text-[11px] transition-colors
                               {settings.vizDefault === val
                                 ? 'bg-[var(--accent)] text-white'
                                 : 'text-[var(--text-secondary)] hover:text-[var(--text-primary)]'}"
                      >{label}</button>
                    {/each}
                  </div>
                </div>
                <!-- Limiter auto -->
                <label class="flex items-center justify-between gap-2 cursor-pointer">
                  <span class="text-[12px] text-[var(--text-primary)]">Auto-enable limiter</span>
                  <input type="checkbox" bind:checked={settings.limiterAuto}
                         class="w-3.5 h-3.5 accent-[var(--accent)]" />
                </label>
                <!-- Auto-collapse queue -->
                <label class="flex items-center justify-between gap-2 cursor-pointer">
                  <span class="text-[12px] text-[var(--text-primary)]">Auto-collapse large queue</span>
                  <input type="checkbox" bind:checked={settings.autoCompact}
                         class="w-3.5 h-3.5 accent-[var(--accent)]" />
                </label>
                <!-- File type column toggle — off = ext merged into filename (gray) -->
                <label class="flex items-center justify-between gap-2 cursor-pointer">
                  <span class="text-[12px] text-[var(--text-primary)]">File type column</span>
                  <input type="checkbox" bind:checked={settings.fileTypeColumn}
                         class="w-3.5 h-3.5 accent-[var(--accent)]" />
                </label>
                <!-- Full bright filetype — off = ext renders in dimmed gray -->
                <label class="flex items-center justify-between gap-2 cursor-pointer">
                  <span class="text-[12px] text-[var(--text-primary)]">Full bright filetype</span>
                  <input type="checkbox" bind:checked={settings.brightFiletype}
                         class="w-3.5 h-3.5 accent-[var(--accent)]" />
                </label>
              </div>

              <!-- Section: Data -->
              <div class="px-4 pt-3 pb-4 flex items-center gap-3 flex-nowrap">
                <!-- svelte-ignore a11y_missing_attribute -->
                <a onclick={() => { settingsOpen = false; aboutOpen = true; }}
                   class="text-[11px] text-blue-400 underline decoration-blue-400/50 hover:decoration-blue-400 cursor-pointer transition-all select-none shrink-0">
                  About
                </a>
                <span class="text-[10px] text-[var(--text-secondary)]/60 select-none shrink-0">Fade{appVersion ? ` v${appVersion}` : ''}</span>
                <div class="flex-1"></div>
                <button
                  onclick={() => { queueManagerEl?.clearPreloadCache(); }}
                  class="shrink-0 px-2.5 py-1 rounded text-[11px] border border-[var(--border)]
                         text-[var(--text-secondary)] hover:text-[var(--text-primary)] hover:border-[var(--accent)]
                         transition-colors whitespace-nowrap">
                  Clear Cache
                </button>
              </div>
            </div>
          {/if}

          <button
            onclick={() => settingsOpen = !settingsOpen}
            class="flex items-center gap-2 px-2.5 py-1.5 rounded text-[12px] border border-[var(--border)]
                   text-[var(--text-secondary)] hover:text-[var(--text-primary)] hover:border-[var(--accent)]
                   transition-colors shrink-0 {settingsOpen ? 'border-[var(--accent)] text-[var(--text-primary)]' : ''}"
          >
            <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor"
                 stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round" class="shrink-0">
              <circle cx="12" cy="12" r="3"/>
              <path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 0 1-2.83 2.83l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-4 0v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 0 1-2.83-2.83l.06-.06A1.65 1.65 0 0 0 4.68 15a1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1 0-4h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 0 1 2.83-2.83l.06.06A1.65 1.65 0 0 0 9 4.68a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 4 0v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 0 1 2.83 2.83l-.06.06A1.65 1.65 0 0 0 19.4 9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 0 4h-.09a1.65 1.65 0 0 0-1.51 1z"/>
            </svg>
            Settings
          </button>

          <!-- Status box: last job/queue outcome. Clicking copies diagnostics.
               Colour coded: gray = info/placeholder, green = success, red = error. -->
          <!-- svelte-ignore a11y_click_events_have_key_events -->
          <!-- svelte-ignore a11y_no_static_element_interactions -->
          <div onclick={copyDiagnostics}
               class="flex-1 min-w-0 px-2.5 flex items-center justify-end rounded cursor-pointer
                      bg-[color-mix(in_srgb,#fff_8%,var(--surface-raised))]
                      hover:bg-[color-mix(in_srgb,#fff_12%,var(--surface-raised))] transition-colors"
               aria-live="polite"
               title="Copy diagnostics to clipboard">
            <span class="text-[11px] truncate text-right
                         {statusKind === 'success' && statusMessage ? 'text-green-400'
                          : statusKind === 'error' && statusMessage ? 'text-red-400'
                          : 'text-gray-400'}">{statusMessage || 'Copy diagnostics.'}</span>
          </div>
        </div>
      </div>

    </aside>

  <!-- Backdrop — click anywhere outside the sidebar to close settings -->
  {#if settingsOpen}
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div class="fixed inset-0 z-[490]" onpointerdown={() => settingsOpen = false}></div>
  {/if}

    <!-- ── CENTER: Preview + timeline ─────────────────────────────────────── -->
    <div class="flex flex-col flex-1 min-w-0 relative z-0">

      <!-- Preview area -->
      <div class="flex-1 min-h-0 bg-[#1a1a1a] flex items-center justify-center relative overflow-hidden" bind:this={previewAreaEl}>

        <!-- DRAFT badge — surfaces auto-flipped draft mode on heavy media
             (video or audio). Informational only; Fade prioritises performance
             over preview fidelity, so there is no toggle back to full quality. -->
        {#if selectedItem && ['video', 'audio'].includes(selectedItem.mediaType) && isHeavyItem(selectedItem)}
          <div
            title="Large file — previews running in draft mode for performance"
            class="absolute top-2 left-2 z-20 px-2 py-0.5 rounded text-[11px] font-mono
                   border backdrop-blur-sm select-none pointer-events-none
                   bg-black/50 border-white/10 text-white/50"
          >DRAFT</div>
        {/if}

        <!-- ── Codec / format constraint warning — shown above the video ── -->
        {#if selectedItem?.mediaType === 'video' && activeOutputCategory === 'video'}
          {@const _wc = videoOptions.codec}
          {@const _wf = videoOptions.output_format}
          {#if ['dnxhd','cineform','dvvideo','rawvideo','wmv2','rv20','xdcam422','xdcam35'].includes(_wc) || _wf === '3gp' || _wf === 'divx'}
            <div class="absolute top-3 left-1/2 -translate-x-1/2 z-30 w-[min(calc(100%-2rem),560px)]">
              <CodecWarning
                codec={_wc}
                output_format={_wf}
                inputWidth={selectedWidth}
                inputHeight={selectedHeight}
                bind:resolution={videoOptions.resolution}
              />
            </div>
          {/if}
        {/if}

        <!-- ── VIDEO: lives outside {#key} so videoEl is NEVER null while a
               video is selected — prevents Timeline falling back to new Audio() ── -->
        {#if selectedItem?.mediaType === 'video'}
          <!-- svelte-ignore a11y_media_has_caption -->
          <video
            bind:this={videoEl}
            src={liveSrc ?? undefined}
            preload="auto"
            onloadedmetadata={onVideoMetaLoaded}
            class="{operationsMode ? 'absolute bottom-4 left-1/2 -translate-x-1/2 w-[560px] h-[316px] rounded-lg shadow-2xl border border-white/10 z-20 object-cover' : 'absolute bottom-4 left-1/2 -translate-x-1/2 max-w-[min(100%-2rem,560px)] max-h-[min(100%-2rem,316px)] w-auto h-auto rounded-lg shadow-2xl border border-white/10 z-10 object-contain'} {operationsMode ? (!liveSrc ? 'hidden' : '') : ((!liveSrc || diffClipPath) ? 'hidden' : '')}"
          ></video>
          {#if !operationsMode}
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
                class="absolute bottom-4 left-1/2 -translate-x-1/2 max-w-[min(100%-2rem,560px)] max-h-[min(100%-2rem,316px)] w-auto h-auto rounded-lg shadow-2xl border border-white/10 z-10 object-contain"
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
        {/if}

        <!-- ── OPERATIONS MODE panel ─────────────────────────────────── -->
        {#if operationsMode}
          <OperationsPanel
            bind:selectedItem
            bind:queue
            bind:replaceAudioPath
            selectedOperation={selectedOperation}
            videoOptions={videoOptions}
            audioOptions={audioOptions}
            outputDir={outputDir}
            outputSeparator={outputSeparator}
            setStatus={setStatus}
            pickAuxFile={pickAuxFile}
            onBack={exitOperationsMode}
          />
        {/if}

        <!-- ── NON-VIDEO content: key block remounts on each selection ── -->
        {#if !operationsMode}
        {#key selectedItem?.id}
          {#if selectedItem?.kind === 'folder'}
            <!-- Batch folder configuration panel — UI only. Wiring up rename
                 and output-routing logic happens in a follow-up. -->
            {@const bf = selectedItem}
            {@const opts = bf.batchOptions}
            <div class="w-full h-full select-none overflow-y-auto px-10 py-8">
              <div class="max-w-2xl mx-auto flex flex-col gap-6">
                <!-- Header -->
                <div class="flex items-center gap-3">
                  <svg width="22" height="22" viewBox="0 0 24 24" fill="none" stroke="currentColor"
                       stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"
                       class="text-[var(--accent)]">
                    <path d="M21 16V8a2 2 0 0 0-1-1.73l-7-4a2 2 0 0 0-2 0l-7 4A2 2 0 0 0 3 8v8a2 2 0 0 0 1 1.73l7 4a2 2 0 0 0 2 0l7-4A2 2 0 0 0 21 16z"/>
                    <polyline points="3.27 6.96 12 12.01 20.73 6.96"/>
                    <line x1="12" y1="22.08" x2="12" y2="12"/>
                  </svg>
                  <input
                    type="text"
                    bind:value={bf.name}
                    class="flex-1 bg-transparent border-b border-transparent hover:border-[var(--border)]
                           focus:border-[var(--accent)] focus:outline-none text-[18px] font-semibold text-white/85 py-1"
                  />
                  <span class="text-[11px] text-white/35 font-mono">
                    {queue.filter(q => q.kind === 'file' && q.parentId === bf.id).length} files
                  </span>
                </div>
                <!-- Add files button -->
                <button
                  onclick={() => onProxyBrowse(bf.id)}
                  class="self-start flex items-center gap-2 px-3 py-1.5 rounded border border-[var(--border)]
                         text-[12px] text-white/70 hover:bg-[var(--accent)] hover:text-white hover:border-[color-mix(in_srgb,var(--accent)_70%,#000)]
                         transition-colors"
                >
                  <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor"
                       stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                    <line x1="12" y1="5" x2="12" y2="19"/>
                    <line x1="5" y1="12" x2="19" y2="12"/>
                  </svg>
                  Add files / folder
                </button>
                <!-- Quick tutorial — sits right below the rename input -->
                <div class="-mt-3 rounded-md border border-white/10 bg-white/[0.03] px-4 py-3">
                  <p class="text-[11px] uppercase tracking-wider text-white/35 font-semibold mb-1.5">
                    What this tool does
                  </p>
                  <ul class="flex flex-col gap-1 text-[12px] text-white/65">
                    <li class="flex gap-2"><span class="text-[var(--accent)]">•</span> Careful renaming with prefixes, suffixes, or custom patterns</li>
                    <li class="flex gap-2"><span class="text-[var(--accent)]">•</span> Batch processing of every file inside the folder with one set of rules</li>
                    <li class="flex gap-2"><span class="text-[var(--accent)]">•</span> Proxy creation that mirrors source folders or writes back in place</li>
                  </ul>
                </div>

                <!-- Output destination -->
                <section class="flex flex-col gap-2">
                  <h3 class="text-[11px] font-semibold uppercase tracking-wider text-white/45">
                    Output destination
                  </h3>
                  <div class="grid grid-cols-1 gap-1.5">
                    {#each [
                      { v: 'mirror',  t: 'Mirror to another drive', d: 'Recreate the source folder structure under a new root.' },
                      { v: 'inplace', t: 'In-place proxies',        d: 'Write outputs back into the same folders as the source files.' },
                      { v: 'flat',    t: 'Flat output folder',      d: 'Dump every output into one destination folder.' },
                    ] as o}
                      <label class="flex items-start gap-2 px-3 py-2 rounded border cursor-pointer transition-colors
                                    {opts.outputMode === o.v
                                      ? 'border-[var(--accent)] bg-[var(--accent)]/10'
                                      : 'border-[var(--border)] hover:border-white/25'}">
                        <input type="radio" bind:group={opts.outputMode} value={o.v} class="mt-1 accent-[var(--accent)]" />
                        <div class="flex flex-col">
                          <span class="text-[12px] font-medium text-white/85">{o.t}</span>
                          <span class="text-[11px] text-white/45">{o.d}</span>
                        </div>
                      </label>
                    {/each}
                  </div>
                  {#if opts.outputMode !== 'inplace'}
                    <div class="flex items-stretch gap-1.5 mt-1">
                      <input
                        type="text"
                        bind:value={opts.outputRoot}
                        placeholder={opts.outputMode === 'mirror' ? 'New root drive / folder…' : 'Destination folder…'}
                        class="flex-1 px-2 py-1.5 rounded border border-[var(--border)] bg-black/30
                               text-[12px] text-white/85 font-mono placeholder-white/25 focus:outline-none
                               focus:border-[var(--accent)]"
                      />
                      <button
                        class="px-3 py-1.5 rounded border border-[var(--border)] text-[12px] text-white/70
                               hover:bg-[var(--accent)] hover:text-white hover:border-[color-mix(in_srgb,var(--accent)_70%,#000)] transition-colors"
                      >Browse…</button>
                    </div>
                  {/if}
                  {#if opts.outputMode === 'mirror'}
                    <label class="flex items-center gap-2 mt-1 cursor-pointer">
                      <input type="checkbox" bind:checked={opts.preserveStructure} class="accent-[var(--accent)]" />
                      <span class="text-[11px] text-white/60">Preserve source folder hierarchy</span>
                    </label>
                  {/if}
                </section>

                <!-- Rename rules -->
                <section class="flex flex-col gap-2">
                  <h3 class="text-[11px] font-semibold uppercase tracking-wider text-white/45">
                    Rename rules
                  </h3>
                  <div class="grid grid-cols-2 gap-1.5">
                    {#each [
                      { v: 'keep',    t: 'Keep original name' },
                      { v: 'suffix',  t: 'Append suffix' },
                      { v: 'prefix',  t: 'Prepend prefix' },
                      { v: 'pattern', t: 'Custom pattern' },
                    ] as o}
                      <label class="flex items-center gap-2 px-3 py-2 rounded border cursor-pointer transition-colors
                                    {opts.renameMode === o.v
                                      ? 'border-[var(--accent)] bg-[var(--accent)]/10'
                                      : 'border-[var(--border)] hover:border-white/25'}">
                        <input type="radio" bind:group={opts.renameMode} value={o.v} class="accent-[var(--accent)]" />
                        <span class="text-[12px] text-white/85">{o.t}</span>
                      </label>
                    {/each}
                  </div>
                  {#if opts.renameMode === 'suffix' || opts.renameMode === 'prefix'}
                    <input
                      type="text"
                      bind:value={opts.renameToken}
                      placeholder={opts.renameMode === 'suffix' ? '_proxy' : 'proxy_'}
                      class="px-2 py-1.5 rounded border border-[var(--border)] bg-black/30
                             text-[12px] text-white/85 font-mono placeholder-white/25 focus:outline-none
                             focus:border-[var(--accent)]"
                    />
                  {:else if opts.renameMode === 'pattern'}
                    <input
                      type="text"
                      bind:value={opts.renamePattern}
                      placeholder="{'{name}_{n}'}"
                      class="px-2 py-1.5 rounded border border-[var(--border)] bg-black/30
                             text-[12px] text-white/85 font-mono placeholder-white/25 focus:outline-none
                             focus:border-[var(--accent)]"
                    />
                    <p class="text-[10px] text-white/35 font-mono">
                      Tokens: {'{name}'} {'{ext}'} {'{n}'} {'{date}'} {'{parent}'}
                    </p>
                  {/if}
                </section>

                <p class="text-[10px] text-white/25 italic">
                  Folder-level rendering is not wired yet — these controls will drive batch output once implemented.
                </p>
              </div>
            </div>
          {:else if selectedItem?.mediaType === 'image' && liveSrc}
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
            <CropEditor
              bind:this={cropEditorEl}
              bind:imageOptions
              bind:cropActive
              bind:cropAspect
              {imgEl}
              {imgNaturalW}
              {imgNaturalH}
              {previewAreaEl}
              selectedId={selectedItem?.id ?? null}
            />
          {:else if selectedItem && selectedItem.kind !== 'folder' && !['video','audio','image'].includes(selectedItem.mediaType)}
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
            <div class="w-full h-full overflow-y-auto flex flex-col items-center px-10 py-10 select-none">
              <!-- Drop prompt -->
              <svg width="28" height="28" viewBox="0 0 24 24" fill="none" stroke="currentColor"
                   stroke-width="1.2" stroke-linecap="round" stroke-linejoin="round"
                   class="text-white/12 mb-3 shrink-0">
                <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"/>
                <polyline points="17 8 12 3 7 8"/>
                <line x1="12" y1="3" x2="12" y2="15"/>
              </svg>
              <p class="text-white/25 text-[13px] font-medium mb-1">Drop files or click Browse</p>
              <p class="text-white/12 text-[11px] mb-8">Select a file in the queue to preview and configure</p>

              <!-- Input formats -->
              <div class="w-full max-w-xl mb-8">
                <p class="text-[18px] font-semibold mb-3"
                   style="color:rgba(255,255,255,0.34)">Supported Input Formats</p>
                <div class="flex flex-col gap-2.5">
                  {#each [
                    { label: 'Image',           exts: ['jpg','jpeg','png','gif','webp','avif','bmp','svg','ico'] },
                    { label: 'RAW / Pro Image', exts: ['heic','heif','tiff','psd','raw','cr2','cr3','nef','arw','dng','orf','rw2','exr','hdr','dds','xcf'] },
                    { label: 'Video',           exts: ['mp4','m4v','mkv','webm','mov','avi','flv','wmv','mpg','mpeg','ogv','ts','3gp','divx','rmvb','asf'] },
                    { label: 'Audio',           exts: ['mp3','aac','ogg','wav','flac','m4a','opus','wma','aiff','alac','ac3','dts'] },
                    { label: 'Document',        exts: ['pdf'] },
                    { label: '3D Model',        exts: ['obj','gltf','glb','stl','fbx','ply','3ds','dae','x3d'] },
                    { label: 'Archive',         exts: ['zip','tar','gz','7z'] },
                    { label: 'Data',            exts: ['json','csv','tsv','xml','yaml'] },
                  ] as g}
                    <div class="flex gap-3 items-baseline">
                      <span class="shrink-0 text-[10px] font-semibold w-28 text-right"
                            style="color:rgba(255,255,255,0.22)">{g.label}</span>
                      <span class="text-[10px] font-mono leading-relaxed">
                        {#each g.exts as ext, i}
                          <span style="color:rgba(255,255,255,0.44)">{ext}</span>{#if i < g.exts.length - 1}{'  '}{/if}
                        {/each}
                      </span>
                    </div>
                  {/each}
                </div>
              </div>

              <!-- Output formats -->
              <div class="w-full max-w-xl">
                <p class="text-[18px] font-semibold mb-3"
                   style="color:rgba(255,255,255,0.34)">Output Formats</p>
                <div class="flex flex-col gap-2.5">
                  {#each [
                    { label: 'Audio',    exts: ['mp3','wav','flac','ogg','aac','opus','m4a','wma','aiff','alac','ac3','dts'] },
                    { label: 'Video',    exts: ['mp4','mov','webm','mkv','avi','gif'] },
                    { label: 'Image',    exts: ['jpeg','png','webp','tiff','bmp','avif'] },
                    { label: 'Document', exts: ['html','pdf','txt','md'] },
                    { label: 'Data',     exts: ['json','csv','tsv','xml','yaml'] },
                    { label: 'Archive',  exts: ['zip','tar','gz','7z'] },
                  ] as g}
                    <div class="flex gap-3 items-baseline">
                      <span class="shrink-0 text-[10px] font-semibold w-28 text-right"
                            style="color:rgba(255,255,255,0.22)">{g.label}</span>
                      <span class="text-[10px] font-mono leading-relaxed">
                        {#each g.exts as ext, i}
                          <span style="color:rgba(255,255,255,0.33)">{ext}</span>{#if i < g.exts.length - 1}{'  '}{/if}
                        {/each}
                      </span>
                    </div>
                  {/each}
                </div>
              </div>
            </div>
          {/if}
          <!-- Loading text — shown while preview is clearing/loading -->
          {#if previewLoading}
            <p class="absolute select-none text-[40px] font-medium"
               style="color:rgba(255,255,255,0.25)">Loading</p>
          {/if}
        {/key}
        {/if}
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
        <Timeline item={selectedItem} duration={selectedDuration} bind:options={videoOptions} mediaEl={videoEl} onscrubstart={dismissDiff} bind:vizExpanded mediaReady={tlMediaReady} waveformReady={tlWaveformReady} spectrogramReady={tlSpectrogramReady} filmstripReady={tlFilmstripReady} cachedWaveform={cachedWaveformForTimeline} cachedFilmstripFrames={cachedFilmstripForTimeline} draft={isHeavyItem(selectedItem)} replacedAudioMode={operationsMode && selectedOperation === 'replace-audio' && !!replaceAudioPath} analysisMode={operationsMode && ['loudness','audio-norm','cut-detect','black-detect','vmaf','framemd5'].includes(selectedOperation)} analysisHistogramDim={operationsMode && ['loudness','audio-norm'].includes(selectedOperation)} />
      {:else if selectedItem?.mediaType === 'audio'}
        <Timeline item={selectedItem} duration={selectedDuration} bind:options={audioOptions} onscrubstart={dismissDiff} bind:vizExpanded mediaReady={tlMediaReady} waveformReady={tlWaveformReady} spectrogramReady={tlSpectrogramReady} cachedWaveform={cachedWaveformForTimeline} draft={isHeavyItem(selectedItem)} replacedAudioMode={operationsMode && selectedOperation === 'replace-audio' && !!replaceAudioPath} analysisMode={operationsMode && ['loudness','audio-norm','cut-detect','black-detect','vmaf','framemd5'].includes(selectedOperation)} analysisHistogramDim={operationsMode && ['loudness','audio-norm'].includes(selectedOperation)} />
      {:else if operationsMode}
        <!-- Placeholder strip — preserves timeline space in ops mode when no media is selected -->
        <div class="shrink-0 h-[180px] bg-[#0f0f0f] border-t border-[var(--border)]
                    flex items-center justify-center text-[11px] text-white/20 select-none">
          Timeline · filmstrip · waveform will appear once a video is selected
        </div>
      {/if}

    </div>

    <!-- ── RIGHT: Options panel (333px) ─────────────────────────────────────── -->
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <aside class="w-[333px] shrink-0 border-l border-[var(--border)] flex flex-col bg-[var(--surface-raised)] relative"
           role="region" aria-label="Conversion options"
           onmouseover={onPanelMouseOver}
           onfocus={onPanelMouseOver}
           onmouseleave={() => setHint('')}
           onblur={() => setHint('')}>

      {#if operationsMode}
        <!-- Operations mode: list takes over the sidebar.
             Header hosts a second copy of the center-panel Back button so the
             user can always reach it on either side. -->
        <div class="flex items-center gap-2 px-3 py-2.5 shrink-0 border-b border-[var(--accent)]/30"
             style="background:color-mix(in srgb, var(--accent) 8%, var(--surface-raised))">
          <button
            onclick={exitOperationsMode}
            data-back-button
            class="px-3 py-1.5 rounded text-[11px] font-semibold bg-[var(--accent)] text-white
                   hover:opacity-90 transition-opacity shrink-0"
          >← Back</button>
        </div>
        <!-- Search — narrows the ops list below -->
        <div class="shrink-0 px-2 py-1.5 border-b border-[var(--border)]"
             style="background:color-mix(in srgb, var(--surface-raised) 60%, #000 40%)">
          <div class="relative">
            <svg class="absolute left-2 top-1/2 -translate-y-1/2 text-[var(--text-secondary)] pointer-events-none"
                 width="11" height="11" viewBox="0 0 16 16" fill="none" stroke="currentColor"
                 stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round">
              <circle cx="7" cy="7" r="5"/>
              <path d="M14 14l-3.5-3.5"/>
            </svg>
            <input
              type="text"
              bind:value={rightSearch}
              placeholder=""
              class="w-full pl-7 pr-6 py-1 text-[11px] rounded border border-[var(--border)]
                     bg-[color:color-mix(in_srgb,#000_40%,var(--surface-raised))]
                     text-[var(--text-primary)] placeholder:text-[var(--text-secondary)]
                     focus:outline-none focus:border-[var(--accent)]"
            />
            {#if rightSearch}
              <button
                onclick={() => rightSearch = ''}
                title="Clear filter"
                class="absolute right-1.5 top-1/2 -translate-y-1/2 text-[var(--text-secondary)] hover:text-[var(--text-primary)] text-[13px] leading-none"
              >×</button>
            {/if}
          </div>
        </div>
        {@const opSearchQ = rightSearch.trim().toLowerCase()}
        <div class="flex-1 overflow-y-auto p-2 flex flex-col gap-0.5">
          {#each OPERATIONS.filter(o => !opSearchQ || o.label.toLowerCase().includes(opSearchQ) || o.id.toLowerCase().includes(opSearchQ)) as op}
            <button
              onclick={() => selectedOperation = op.id}
              class="w-full px-3 py-2 rounded text-[12px] text-left font-medium border transition-colors
                     {selectedOperation === op.id
                       ? 'bg-[var(--accent)] text-white border-[var(--accent)]'
                       : 'border-transparent text-white hover:bg-white/5 hover:border-[var(--border)]'}"
            >{op.label}</button>
          {/each}
        </div>
      {:else}
      <!-- ── Panel header: Output picker + Presets — the "control plane"
           for the options panel. Always docked (shrink-0 + no scroll wrapper).
           Subtle accent tint + heavier border make it visually distinct from
           the option fields below. ────────────────────────────────────── -->
      <div class="flex items-center gap-2 px-3 py-1.5 shrink-0 relative"
           style="background:color-mix(in srgb, var(--accent) 6%, var(--surface-raised));
                  border-bottom:1px solid color-mix(in srgb, var(--accent) 45%, var(--border))">

        <!-- Output format control. Two modes:
             - No format selected → "Output" picker-toggle (drops user into the
               format picker grid below).
             - Format selected → "← Back" button mirroring the operations-page
               Back button; returns user to the picker. The active format's
               name is surfaced as a large centered page title below the
               header instead, where it's far more legible. -->
        {#if globalOutputFormat}
          {@const _fmt = FORMAT_GROUPS.find(g => g.fmts.some(f => f.id === globalOutputFormat))?.fmts.find(f => f.id === globalOutputFormat)}
          <button
            onclick={() => { globalOutputFormat = null; }}
            data-tooltip="Back to the format picker grid."
            data-back-button
            class="px-3 py-1.5 rounded text-[11px] font-semibold bg-[var(--accent)] text-white
                   hover:opacity-90 transition-opacity shrink-0
                   {presetsMode ? 'invisible pointer-events-none' : ''}"
            aria-hidden={presetsMode}
            tabindex={presetsMode ? -1 : 0}
          >← Back</button>
          <h2 class="flex-1 min-w-0 text-center text-[14px] font-semibold text-white/85 truncate">
            {_fmt?.label ?? globalOutputFormat.toUpperCase()}
          </h2>
        {/if}

        <!-- Presets selector — always visible; filtered to active category when one is selected -->
        <PresetManager
          bind:this={presetManagerEl}
          bind:imageOptions
          bind:videoOptions
          bind:audioOptions
          bind:globalOutputFormat
          bind:presetsMode
          bind:combinedPresets={presetsList}
          {activeOutputCategory}
          {setStatus}
        />
      </div>

      <!-- Search — filters the format/tool grid below. Only visible on the
           picker page (when globalOutputFormat is null) so it doesn't clutter
           the options pages. -->
      {#if !globalOutputFormat}
        <div class="shrink-0 px-2 py-1.5 border-b border-[var(--border)]"
             style="background:color-mix(in srgb, var(--surface-raised) 60%, #000 40%)">
          <div class="relative">
            <svg class="absolute left-2 top-1/2 -translate-y-1/2 text-[var(--text-secondary)] pointer-events-none"
                 width="11" height="11" viewBox="0 0 16 16" fill="none" stroke="currentColor"
                 stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round">
              <circle cx="7" cy="7" r="5"/>
              <path d="M14 14l-3.5-3.5"/>
            </svg>
            <input
              type="text"
              bind:value={rightSearch}
              placeholder=""
              class="w-full pl-7 pr-6 py-1 text-[11px] rounded border border-[var(--border)]
                     bg-[color:color-mix(in_srgb,#000_40%,var(--surface-raised))]
                     text-[var(--text-primary)] placeholder:text-[var(--text-secondary)]
                     focus:outline-none focus:border-[var(--accent)]"
            />
            {#if rightSearch}
              <button
                onclick={() => rightSearch = ''}
                title="Clear filter"
                class="absolute right-1.5 top-1/2 -translate-y-1/2 text-[var(--text-secondary)] hover:text-[var(--text-primary)] text-[13px] leading-none"
              >×</button>
            {/if}
          </div>
        </div>
      {/if}

      <!-- Options content -->
      <div class="fade-scroll-stable flex-1 min-h-0 overflow-y-scroll p-4{settings.showDevFeatures ? ' dev-text-audit' : ''}">
        {#if presetsMode}
          <!-- ── Preset picker — mirrors format picker layout, presets in place of formats.
               When a format is already selected, the list is narrowed to that media category. ── -->
          {@const OPS_CATS  = ['intact', 'processing']}
          {@const TOOL_CATS = [...OPS_CATS, 'chroma', 'ai', 'analysis', 'burn']}
          {@const FILE_CATS = ['data', 'document', 'archive', 'office', 'ebook', 'subtitle', 'timeline', 'font', 'email']}
          {@const conversionGroups = sortedFormatGroups
              .filter(g => !TOOL_CATS.includes(g.cat) && !FILE_CATS.includes(g.cat))
              .filter(g => !globalOutputFormat || g.cat === activeOutputCategory)}
          {@const _presetSearchQ = rightSearch.trim().toLowerCase()}
          <div class="flex flex-col gap-5">
            <section>
              <button
                onclick={() => settings.conversionCollapsed = !settings.conversionCollapsed}
                class="w-full flex items-center gap-2 mb-3 group"
              >
                <span class="text-[13px] font-semibold uppercase tracking-wider text-white">Media</span>
                <div class="flex-1 h-px bg-[var(--border)]"></div>
                <svg width="10" height="10" viewBox="0 0 10 10" fill="none" stroke="currentColor"
                     stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"
                     class="text-[var(--text-secondary)] group-hover:text-[var(--text-primary)] transition-transform duration-150
                            {settings.conversionCollapsed ? '-rotate-90' : ''}">
                  <path d="M2 4l3 3 3-3"/>
                </svg>
              </button>
              {#if !settings.conversionCollapsed}
                <div class="space-y-4">
                  {#each conversionGroups as group (group.cat)}
                    {@const _presets = presetsList.filter(p => p.media_type === group.cat && (!_presetSearchQ || p.name.toLowerCase().includes(_presetSearchQ)))}
                    <div>
                      <div class="flex items-center gap-2 mb-1.5">
                        <span class="text-[9px] font-semibold uppercase tracking-wider
                                     {_presets.length > 0 ? 'text-[var(--text-secondary)]' : 'text-[var(--text-secondary)]/25'}">
                          {group.label}
                        </span>
                        <div class="flex-1 h-px {_presets.length > 0 ? 'bg-[var(--border)]' : 'bg-[var(--border)]/25'}"></div>
                      </div>
                      {#if _presets.length > 0}
                        <div class="flex flex-wrap gap-1">
                          {#each _presets as p (p.id)}
                            <button
                              onclick={() => { presetManagerEl.applyPreset(p.id); presetsMode = false; }}
                              class="px-2 py-0.5 rounded text-[11px] border transition-colors
                                     border-[var(--border)] text-[var(--text-primary)]
                                     hover:bg-[var(--accent)] hover:text-white hover:border-[color-mix(in_srgb,var(--accent)_70%,#000)]"
                            >{p.name}</button>
                          {/each}
                        </div>
                      {/if}
                    </div>
                  {/each}
                </div>
              {/if}
            </section>
          </div>
        {:else if !globalOutputFormat}
          {@const OPS_CATS  = ['intact', 'processing']}
          {@const TOOL_CATS = [...OPS_CATS, 'chroma', 'ai', 'analysis', 'burn']}
          {@const FILE_CATS = ['data', 'document', 'archive', 'office', 'ebook', 'subtitle', 'timeline', 'font', 'email']}
          {@const searchQ = rightSearch.trim().toLowerCase()}
          {@const matchesSearch = (f) => !searchQ || (f.label ?? f.id ?? '').toLowerCase().includes(searchQ) || (f.id ?? '').toLowerCase().includes(searchQ)}
          {@const conversionGroups = sortedFormatGroups.filter(g => !TOOL_CATS.includes(g.cat) && !FILE_CATS.includes(g.cat))}
          {@const toolGroups       = sortedFormatGroups.filter(g =>  TOOL_CATS.includes(g.cat))}
          {@const fileGroups       = sortedFormatGroups.filter(g =>  FILE_CATS.includes(g.cat))}
          <!-- ── Format picker: two super-sections (Conversion / Tools) ───── -->
          <div class="flex flex-col gap-5">

            <!-- ── CONVERSION super-section ─────────────────────────────── -->
            <section>
              <button
                onclick={() => settings.conversionCollapsed = !settings.conversionCollapsed}
                class="w-full flex items-center gap-2 mb-3 group"
              >
                <span class="text-[13px] font-semibold uppercase tracking-wider text-white">Media</span>
                <div class="flex-1 h-px bg-[var(--border)]"></div>
                <svg width="10" height="10" viewBox="0 0 10 10" fill="none" stroke="currentColor"
                     stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"
                     class="text-[var(--text-secondary)] group-hover:text-[var(--text-primary)] transition-transform duration-150
                            {settings.conversionCollapsed ? '-rotate-90' : ''}">
                  <path d="M2 4l3 3 3-3"/>
                </svg>
              </button>
              {#if !settings.conversionCollapsed}
                <div class="space-y-4">
                  {#each conversionGroups as group (group.cat)}
                    {@const _fmts = group.fmts.filter(f => {
                        if (!(!f.todo || (f.preview && settings.showDevFeatures))) return false;
                        if (!matchesSearch(f)) return false;
                        const entryCat = f.cat ?? group.cat;
                        if (compatibleOutputCats !== null && !compatibleOutputCats.includes(entryCat === 'codec' ? 'video' : entryCat)) return false;
                        return true;
                      })}
                    {#if _fmts.length > 0}
                    <div>
                      <div class="flex items-center gap-2 mb-1.5">
                        <span class="text-[9px] font-semibold uppercase tracking-wider text-[var(--text-secondary)]">
                          {group.label}
                        </span>
                        <div class="flex-1 h-px bg-[var(--border)]"></div>
                      </div>
                      <div class="flex flex-wrap gap-1">
                        {#each _fmts as f}
                          <button
                            onclick={() => {
                              if (group.cat === 'codec') {
                                // Codec preset: set both the container AND the codec so the
                                // Video options panel lands on the right combo immediately.
                                globalOutputFormat = f.ext;
                                videoOptions.output_format = f.ext;
                                videoOptions.codec = f.codec;
                              } else {
                                globalOutputFormat = f.id;
                                // Auto-select a compatible codec for containers that
                                // reject arbitrary codecs or need specific defaults.
                                const fmt = f.id;
                                if      (fmt === 'ogv')               videoOptions.codec = 'theora';
                                else if (fmt === 'mpg')               videoOptions.codec = 'mpeg2video';
                                else if (fmt === 'wmv' || fmt === 'asf') videoOptions.codec = 'wmv2';
                                else if (fmt === 'divx')              videoOptions.codec = 'mpeg4';
                                else if (fmt === 'rmvb')              videoOptions.codec = 'rv20';
                                else if (fmt === '3gp') { videoOptions.codec = 'h264'; videoOptions.h264_profile = 'baseline'; }
                              }
                            }}
                            data-tooltip={group.cat === 'codec' ? `${f.label} — encode as ${f.codec} in ${f.ext.toUpperCase()}` : `Convert to ${(f.label ?? f.id).toUpperCase()} — ${group.label.toLowerCase()} output`}
                            class="px-2 py-0.5 rounded text-[11px] font-mono border transition-colors
                                   {f.todo
                                     ? 'border-green-900 text-green-400 hover:border-green-600 hover:bg-green-950'
                                     : 'border-[var(--border)] text-[var(--text-primary)] hover:bg-[var(--accent)] hover:text-white hover:border-[color-mix(in_srgb,var(--accent)_70%,#000)]'}"
                          >{f.label ?? f.id}</button>
                        {/each}
                      </div>
                    </div>
                    {/if}
                  {/each}
                </div>
              {/if}
            </section>

            <!-- ── TOOLS super-section ──────────────────────────────────── -->
            <section>
              <button
                onclick={() => settings.toolsCollapsed = !settings.toolsCollapsed}
                class="w-full flex items-center gap-2 mb-3 group"
              >
                <span class="text-[13px] font-semibold uppercase tracking-wider text-white">Tools</span>
                <div class="flex-1 h-px bg-[var(--border)]"></div>
                <svg width="10" height="10" viewBox="0 0 10 10" fill="none" stroke="currentColor"
                     stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"
                     class="text-[var(--text-secondary)] group-hover:text-[var(--text-primary)] transition-transform duration-150
                            {settings.toolsCollapsed ? '-rotate-90' : ''}">
                  <path d="M2 4l3 3 3-3"/>
                </svg>
              </button>
              {#if !settings.toolsCollapsed}
                <div class="space-y-4">
                  {#each toolGroups as group (group.cat)}
                    {@const isOpsGroup = OPS_CATS.includes(group.cat)}
                    {@const _fmts = group.fmts.filter(f => {
                        const isOpsEntry = isOpsGroup || f.ops;
                        if (!(!f.todo || (f.preview && settings.showDevFeatures) || isOpsGroup || f.ops)) return false;
                        if (!matchesSearch(f)) return false;
                        if (!isOpsEntry && compatibleOutputCats !== null && !compatibleOutputCats.includes(group.cat)) return false;
                        return true;
                      })}
                    {#if _fmts.length > 0}
                    <div>
                      <div class="flex items-center gap-2 mb-1.5">
                        <span class="text-[9px] font-semibold uppercase tracking-wider text-[var(--text-secondary)]">
                          {group.label}
                        </span>
                        <div class="flex-1 h-px bg-[var(--border)]"></div>
                      </div>
                      <div class="flex flex-wrap gap-1">
                        {#each _fmts as f}
                          {@const isOpsEntry = isOpsGroup || f.ops}
                          <button
                            onclick={() => {
                              if (isOpsEntry) {
                                // Subtitling is cross-surfaced from 3 categories —
                                // auto-focus the matching tab on the unified page so
                                // the user lands where they expected.
                                if (f.id === 'subtitling') {
                                  subtitlingTab = group.cat === 'ai'       ? 'generate'
                                               : group.cat === 'analysis' ? 'analyze'
                                                                          : 'embed';
                                }
                                enterOperation(f.id);
                              } else { globalOutputFormat = f.id; }
                            }}
                            data-tooltip={isOpsEntry ? `${(f.label ?? f.id)} — open operations mode` : `Convert to ${(f.label ?? f.id).toUpperCase()} — ${group.label.toLowerCase()} output`}
                            class="px-2 py-0.5 rounded text-[11px] font-mono border transition-colors
                                   {f.todo && !isOpsEntry
                                     ? 'border-green-900 text-green-400 hover:border-green-600 hover:bg-green-950'
                                     : 'border-[var(--border)] text-[var(--text-primary)] hover:bg-[var(--accent)] hover:text-white hover:border-[color-mix(in_srgb,var(--accent)_70%,#000)]'}"
                          >{f.label ?? f.id}</button>
                        {/each}
                      </div>
                    </div>
                    {/if}
                  {/each}
                </div>
              {/if}
            </section>

            <!-- ── FILES super-section ──────────────────────────────────── -->
            <section>
              <button
                onclick={() => settings.filesCollapsed = !settings.filesCollapsed}
                class="w-full flex items-center gap-2 mb-3 group"
              >
                <span class="text-[13px] font-semibold uppercase tracking-wider text-white">Files</span>
                <div class="flex-1 h-px bg-[var(--border)]"></div>
                <svg width="10" height="10" viewBox="0 0 10 10" fill="none" stroke="currentColor"
                     stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"
                     class="text-[var(--text-secondary)] group-hover:text-[var(--text-primary)] transition-transform duration-150
                            {settings.filesCollapsed ? '-rotate-90' : ''}">
                  <path d="M2 4l3 3 3-3"/>
                </svg>
              </button>
              {#if !settings.filesCollapsed}
                <div class="space-y-4">
                  {#each fileGroups.filter(g => !g.todo || settings.showDevFeatures) as group (group.cat)}
                    {@const _fmts = group.fmts.filter(f =>
                        (!f.todo || (f.preview && settings.showDevFeatures)) &&
                        matchesSearch(f) &&
                        !(compatibleOutputCats !== null && !compatibleOutputCats.includes(group.cat))
                      )}
                    {#if _fmts.length > 0}
                    <div>
                      <div class="flex items-center gap-2 mb-1.5">
                        <span class="text-[9px] font-semibold uppercase tracking-wider
                                     {group.todo ? 'text-green-600' : 'text-[var(--text-secondary)]'}">
                          {group.label}
                        </span>
                        <div class="flex-1 h-px {group.todo ? 'bg-green-900' : 'bg-[var(--border)]'}"></div>
                      </div>
                      <div class="flex flex-wrap gap-1">
                        {#each _fmts as f}
                          <button
                            onclick={() => { globalOutputFormat = f.id; }}
                            data-tooltip={`Convert to ${(f.label ?? f.id).toUpperCase()} — ${group.label.toLowerCase()} output`}
                            class="px-2 py-0.5 rounded text-[11px] font-mono border transition-colors
                                   {f.todo
                                     ? 'border-green-900 text-green-400 hover:border-green-600 hover:bg-green-950'
                                     : 'border-[var(--border)] text-[var(--text-primary)] hover:bg-[var(--accent)] hover:text-white hover:border-[color-mix(in_srgb,var(--accent)_70%,#000)]'}"
                          >{f.label ?? f.id}</button>
                        {/each}
                      </div>
                    </div>
                    {/if}
                  {/each}
                </div>
              {/if}
            </section>
          </div>
        {:else}
          <div class="dev-audit-ok" style="display:contents">
            {#if activeOutputCategory === 'image'}
              <ImageOptions bind:options={imageOptions}
                onqualitystart={onQualityStart}
                onqualityinput={onQualityInput}
                oncropstart={(aspect) => cropEditorEl?.activate(aspect)}
                oncropclear={() => cropEditorEl?.clear()}
                {cropActive}
                {cropAspect}
              />
            {:else if activeOutputCategory === 'video'}
              <VideoOptions bind:options={videoOptions} errors={validationErrors} />
            {:else if activeOutputCategory === 'audio'}
              <AudioOptions bind:options={audioOptions} errors={validationErrors} />
            {:else if activeOutputCategory === 'data'}
              <DataOptions bind:options={dataOptions} />
            {:else if activeOutputCategory === 'document'}
              <FormatPicker bind:options={documentOptions} formats={DOCUMENT_FORMATS} ariaLabel="Document conversion options" />
            {:else if activeOutputCategory === 'archive'}
              <ArchiveOptions bind:options={archiveOptions} />
            {:else if activeOutputCategory === 'model'}
              <!-- No user-tunable options for 3D model conversion today —
                   assimp picks the format-ID from the chosen output extension.
                   Render nothing rather than plastering a placeholder. -->
            {:else if activeOutputCategory === 'timeline'}
              <FormatPicker bind:options={timelineOptions} formats={TIMELINE_FORMATS} ariaLabel="Timeline conversion options" />
            {:else if activeOutputCategory === 'font'}
              <FormatPicker bind:options={fontOptions} formats={FONT_FORMATS} ariaLabel="Font conversion options" upperCase={false} />
            {:else if activeOutputCategory === 'subtitle'}
              <FormatPicker bind:options={subtitleOptions} formats={SUBTITLE_FORMATS} ariaLabel="Subtitle conversion options" />
            {:else if activeOutputCategory === 'ebook'}
              <FormatPicker bind:options={ebookOptions} formats={EBOOK_FORMATS} ariaLabel="Ebook conversion options" />
            {:else if activeOutputCategory === 'email'}
              <FormatPicker bind:options={emailOptions} formats={EMAIL_FORMATS} ariaLabel="Email conversion options" />
            {:else}
              <div class="flex flex-col items-center justify-center h-full text-center gap-2">
                <p class="text-[11px] text-green-500">Coming soon</p>
              </div>
            {/if}
          </div>
        {/if}
      </div>
      {/if}

      <!-- ── Bottom footer: hint text + zoom controls — always visible ──── -->
      <div class="shrink-0 border-t border-[var(--border)] flex flex-col gap-2 px-3 pt-2.5 pb-1"
           style="background:var(--surface-hint)">

        <!-- Hint text — opacity + transition driven by shared tooltip store
             (see tooltip.svelte.js). Handles 100ms in, 2s hold + 2s out,
             and 100ms crossfade on interrupt. Update-available notice
             layers centered over the same band when Notify is on and no
             tooltip is active; clicking opens Settings. -->
        <div class="relative min-h-[2.5rem]">
          <p class="text-[11px] leading-relaxed"
             style="color:rgba(255,255,255,0.5);
                    opacity:{tooltip.opacity};
                    transition:opacity {tooltip.duration}ms linear {tooltip.delay}ms">
            {tooltip.text}
          </p>
          {#if settings.notifyUpdates && (updateState === 'available' || updateState === 'ready') && tooltip.opacity < 0.05}
            <button
              type="button"
              onclick={() => { settingsOpen = true; }}
              class="absolute inset-0 flex items-center justify-center text-[11px] leading-relaxed
                     bg-transparent border-0 cursor-pointer transition-colors"
              style="color:rgba(255,255,255,0.5)"
              onmouseenter={(e) => e.currentTarget.style.color = 'rgba(255,255,255,0.85)'}
              onmouseleave={(e) => e.currentTarget.style.color = 'rgba(255,255,255,0.5)'}
            >{updateState === 'ready' ? 'Update ready — restart to apply' : 'Update available'}</button>
          {/if}
        </div>

        <!-- Zoom (left) + version (right, tucked low with slow pulse) -->
        <div class="flex items-center justify-between gap-0.5">
          <div class="flex items-center gap-0.5">
          <button
            onclick={(e) => zoomClick(zoom.stepOut, e)}
            data-tooltip="Zoom out the entire UI — ⌘- or Ctrl-"
            title="Zoom out (⌘-)"
            disabled={zoom.level === ZOOM_STEPS[0]}
            class="w-5 h-5 flex items-center justify-center rounded text-[11px] transition-colors
                   bg-white/5 hover:bg-white/10 disabled:opacity-20 disabled:cursor-default"
            style="color:rgba(255,255,255,0.45)">−</button>
          <button
            onclick={(e) => zoomClick(zoom.reset, e)}
            data-tooltip="Reset zoom to 100% — ⌘0 or Ctrl0"
            title="Reset zoom (⌘0)"
            class="px-1.5 h-5 flex items-center justify-center rounded text-[10px] font-mono transition-colors
                   bg-white/5 hover:bg-white/10
                   {zoom.level !== 1.0 ? 'text-white' : ''}"
            style={zoom.level === 1.0 ? 'color:rgba(255,255,255,0.35)' : ''}>
            {Math.round(zoom.level * 100)}%</button>
          <button
            onclick={(e) => zoomClick(zoom.stepIn, e)}
            data-tooltip="Zoom in the entire UI — ⌘+ or Ctrl+"
            title="Zoom in (⌘+)"
            disabled={zoom.level === ZOOM_STEPS[ZOOM_STEPS.length - 1]}
            class="w-5 h-5 flex items-center justify-center rounded text-[11px] transition-colors
                   bg-white/5 hover:bg-white/10 disabled:opacity-20 disabled:cursor-default"
            style="color:rgba(255,255,255,0.45)">+</button>
          </div>
          <button onclick={() => aboutOpen = true}
                  class="fade-pulse text-[10px] font-medium select-none hover:opacity-80 transition-opacity cursor-pointer">Fade {appVersion ? `v${appVersion}` : ''}</button>
        </div>
      </div>

    </aside>

  <!-- About modal -->
  {#if aboutOpen}
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div class="{aboutClosing ? 'about-backdrop-out' : 'about-backdrop'} fixed inset-0 z-[600] flex items-center justify-center"
         onpointerdown={closeAbout}>
      <!-- svelte-ignore a11y_no_static_element_interactions -->
      <div class="{aboutClosing ? 'about-card-out' : 'about-card'} relative rounded-xl border border-[var(--border)] shadow-2xl overflow-hidden"
           style="width:420px; background:var(--surface-raised)"
           onpointerdown={(e) => e.stopPropagation()}>

        <!-- Header band -->
        <div class="px-8 pt-8 pb-6 flex flex-col items-center gap-3 border-b border-[var(--border)]"
             style="background:color-mix(in srgb, var(--accent) 6%, var(--surface-raised))">
          <img src={fadeIconDark} alt="Fade" class="w-16 h-16 rounded-2xl" />
          <div class="text-center">
            <p class="text-[22px] font-semibold text-[var(--text-primary)]">Fade</p>
            {#if appVersion}
              <p class="text-[12px] text-[var(--text-secondary)] mt-0.5">v{appVersion}</p>
            {/if}
          </div>
        </div>

        <!-- Body -->
        <div class="px-8 py-6 flex flex-col gap-4 text-[13px] text-[var(--text-secondary)] leading-relaxed">
          <p>
            Part of the <strong class="text-[var(--text-primary)]">Libre</strong> family of professional tools.
          </p>
          <p>
            By <!-- svelte-ignore a11y_missing_attribute -->
            <a onclick={(e) => { e.stopPropagation(); invoke('open_url', { url: 'https://irontreesoftware.com' }); }}
               class="text-[var(--text-primary)] underline decoration-white/20 hover:decoration-white/60 cursor-pointer transition-all">
              Iron Tree Software
            </a>
          </p>

          <div class="flex flex-col gap-1.5 pt-1 text-[12px] border-t border-[var(--border)]">
            <div class="flex justify-between">
              <span class="text-[var(--text-secondary)]">Engine</span>
              <span class="text-[var(--text-primary)] font-mono">ffmpeg</span>
            </div>
            <div class="flex justify-between">
              <span class="text-[var(--text-secondary)]">Runtime</span>
              <span class="text-[var(--text-primary)] font-mono">Tauri + Svelte</span>
            </div>
            <div class="flex justify-between">
              <span class="text-[var(--text-secondary)]">License</span>
              <span class="text-[var(--text-primary)] font-mono">MIT</span>
            </div>
          </div>
        </div>
      </div>
    </div>
  {/if}

  <!-- Full-window drag overlay -->
  {#if dragOver}
    <div class="absolute inset-0 z-40 flex items-center justify-center
                bg-[var(--accent)]/10 border-2 border-dashed border-[var(--accent)]
                pointer-events-none rounded-sm">
      <p class="text-[var(--accent)] text-lg font-medium">Drop files to add</p>
    </div>
  {/if}

  <!-- ── Overlay dropdown — portaled to document.body to escape every stacking context ── -->
  {#if overlay.open}
    <div {@attach portal}>
      <!-- svelte-ignore a11y_no_static_element_interactions -->
      <div style="position:fixed; inset:0; z-index:2147483646;"
           role="presentation" onmousedown={() => overlay.hide()}></div>
      <div
           onmousedown={(e) => e.stopPropagation()}
           style="position:fixed;
                  left:{overlay.anchorRect?.left ?? 0}px;
                  top:{(overlay.anchorRect?.bottom ?? 0) + 4}px;
                  width:{overlay.anchorRect?.width ?? 120}px;
                  z-index:2147483647;"
      >
        <div class="bg-[var(--surface-panel)] border border-[var(--border)] rounded-lg shadow-xl py-1 animate-fade-in">
          {#each overlay.items as item}
            {#if item === null}
              <div class="my-1 border-t border-[var(--border)]" role="separator"></div>
            {:else}
              <button
                onmousedown={(e) => { e.stopPropagation(); overlay.onPick?.(item.value); overlay.hide(); }}
                class="w-full flex items-center px-3 py-[5px] text-[13px] text-left transition-colors
                       cursor-default outline-none
                       hover:bg-[var(--surface-raised)] hover:text-[var(--text-primary)]
                       text-[color-mix(in_srgb,var(--text-primary)_80%,transparent)]"
              >{item.label}</button>
            {/if}
          {/each}
        </div>
      </div>
    </div>
  {/if}

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

  @keyframes about-backdrop-in {
    0%   { background: rgba(0,0,0,0);   backdrop-filter: blur(0px); }
    100% { background: rgba(0,0,0,0.5); backdrop-filter: blur(7px); }
  }
  .about-backdrop {
    animation: about-backdrop-in 2s cubic-bezier(0.4, 0, 0.2, 1) forwards;
  }

  @keyframes about-card-in {
    0%   { opacity: 0; filter: blur(8px); transform: translateY(12px); }
    100% { opacity: 1; filter: blur(0px); transform: translateY(0); }
  }
  .about-card {
    opacity: 0;
    animation: about-card-in 1s cubic-bezier(0.22, 1, 0.36, 1) forwards;
  }

  @keyframes about-backdrop-out {
    0%   { background: rgba(0,0,0,0.5); backdrop-filter: blur(7px); }
    100% { background: rgba(0,0,0,0);   backdrop-filter: blur(0px); }
  }
  .about-backdrop-out {
    animation: about-backdrop-out 0.5s cubic-bezier(0.4, 0, 0.2, 1) forwards;
  }

  @keyframes about-card-out {
    0%   { opacity: 1; filter: blur(0px); transform: translateY(0); }
    100% { opacity: 0; filter: blur(8px); transform: translateY(12px); }
  }
  .about-card-out {
    animation: about-card-out 0.5s cubic-bezier(0.4, 0, 1, 1) forwards;
  }
  :global(input[type="checkbox"]) {
    transform: scale(1.25);
    transform-origin: center;
  }
</style>
