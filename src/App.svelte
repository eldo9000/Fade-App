<script>
  import { invoke, convertFileSrc } from '@tauri-apps/api/core';
  import { listen } from '@tauri-apps/api/event';
  import { onMount, onDestroy } from 'svelte';
  import { initTheme } from '@libre/ui/src/theme.js';
  import { ProgressBar } from '@libre/ui';
  import Queue from './lib/Queue.svelte';
  import Timeline from './lib/Timeline.svelte';
  import ImageOptions from './lib/ImageOptions.svelte';
  import VideoOptions from './lib/VideoOptions.svelte';
  import CodecWarning from './lib/CodecWarning.svelte';
  import AudioOptions from './lib/AudioOptions.svelte';
  import DataOptions from './lib/DataOptions.svelte';
  import FormatPicker from './lib/FormatPicker.svelte';
  import ArchiveOptions from './lib/ArchiveOptions.svelte';
  import { mediaTypeFor, validateOptions } from './lib/utils.js';
  import { createZoom, ZOOM_STEPS } from './lib/stores/zoom.svelte.js';
  import { tooltip, setHint } from './lib/stores/tooltip.svelte.js';
  import { getCurrentWindow } from '@tauri-apps/api/window';
  import { createSettings } from './lib/stores/settings.svelte.js';
  import { pushError, clearDiagnostics, getEntries as getDiagEntries, snapshotText as diagSnapshot, loadPersisted as loadPersistedDiag, uploadDiagnostics, BETA } from './lib/stores/diagnostics.svelte.js';
  import { getVersion } from '@tauri-apps/api/app';
  import UpdateManager from './lib/UpdateManager.svelte';
  import PresetManager from './lib/PresetManager.svelte';
  import CropEditor from './lib/CropEditor.svelte';

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

  let queue = $state([]);
  let selectedId = $state(null);                  // primary selection — drives the centre panel
  let selectedIds = $state(new Set());            // full multi-selection set (highlighted)
  let selectAnchorId = $state(null);              // shift-range anchor
  let draggingFileId = $state(null);              // intra-app drag (file → folder)
  let folderDropHover = $state(false);            // centre-panel drop-zone hover state
  let selectedItem = $derived(queue.find(q => q.id === selectedId) ?? null);

  // ── Operations mode ────────────────────────────────────────────────────────
  let operationsMode = $state(false);
  let selectedOperation = $state(null);
  let cutMode = $state('cut');   // 'cut' = keep range between handles · 'extract' = remove range between handles
  // Subtitling page tabs — one unified page surfaced from three sidebar categories
  // (Intact Video, AI Tools, Analysis). Tabs keep each workflow separate while
  // preventing the back-and-forth-clicking confusion of parallel pages.
  let subtitlingTab = $state('embed'); // 'embed' | 'generate' | 'analyze'
  let replaceAudioPath = $state(null); // path of replacement audio (Replace Audio op)
  let replaceAudioOffsetMs = $state(0); // audio offset in ms (negative = earlier)
  let replaceAudioFitLength = $state(false); // true = time-stretch replacement to match video length
  let replaceAudioAutoSync = $state(false); // one-shot: xcorr align + pitch-preserved stretch + SR/codec match
  // ── Rewrap op ──────────────────────────────────────────────────────────
  // Output container. 4 buttons in the panel toggle this; Run reads it.
  let rewrapFormat = $state('mp4');      // 'mp4' | 'mkv' | 'mov' | 'webm'
  // ── Extract op ─────────────────────────────────────────────────────────
  // Which streams to pull out. Audio / subtitle modes extract every matching
  // stream into its own file; a per-track picker can layer on later once
  // ffprobe results land in the UI.
  let extractMode = $state('video');     // 'video' | 'audio' | 'subtitle' | 'all'
  // ── Merge op ───────────────────────────────────────────────────────────
  // Ordered list of queue item ids. Add from queue pushes selectedId; the
  // list is rendered below with remove + reorder buttons.
  let mergeSelection = $state([]);       // string[] (queue item ids, display order)
  // ── Conform op ─────────────────────────────────────────────────────────
  let conformFps = $state('23.976');   // '23.976' | '24' | '25' | '29.97' | '30' | '50' | '59.94' | '60' | 'source'
  let conformResolution = $state('source'); // 'source' | '3840x2160' | '1920x1080' | '1280x720' | '854x480'
  let conformPixFmt = $state('yuv420p');    // 'yuv420p' | 'yuv420p10le' | 'yuv422p' | 'yuv422p10le' | 'yuv444p' | 'source'
  let conformFpsAlgo = $state('drop');  // 'drop' (fps filter) · 'blend' (framerate filter) · 'mci' (minterpolate)
  let conformScaleAlgo = $state('lanczos'); // 'bilinear' · 'bicubic' · 'lanczos' · 'spline'
  let conformDither = $state(true);     // 10→8-bit dither (error_diffusion)
  // ── Silence Remover ────────────────────────────────────────────────────
  // ffmpeg `silenceremove` filter. Threshold in dB; min silence duration;
  // optional padding kept around each kept region so speech doesn't clip.
  let silenceThresholdDb = $state(-30);  // -dB below which counts as silence
  let silenceMinDurS     = $state(0.5);  // gaps shorter than this are kept
  let silencePadMs       = $state(100);  // leave N ms of silence around keeps
  // ── Intact stream-copy tools ───────────────────────────────────────────
  let metadataStripMode  = $state('all');      // 'all' · 'title'
  let metadataStripTitle = $state('');         // new title when mode==='title'
  let loopCount          = $state(2);          // 2..=50 total playthroughs
  // ── Video processing filters ───────────────────────────────────────────
  let rotateFlipMode     = $state('cw90');     // 'cw90' · 'ccw90' · '180' · 'hflip' · 'vflip'
  let speedPreset        = $state('1');        // preset id: '0.5'|'0.75'|'1'|'1.25'|'1.5'|'2'|'custom'
  let speedCustom        = $state(1.0);
  let fadeInS            = $state(0.5);
  let fadeOutS           = $state(0.5);
  let deinterlaceMode    = $state('yadif');    // 'yadif' · 'yadif_double' · 'bwdif'
  let denoisePreset      = $state('medium');   // 'light' · 'medium' · 'strong'
  // ── Frame / image tools ────────────────────────────────────────────────
  let thumbnailTime      = $state('00:00:01');
  let thumbnailFormat    = $state('jpeg');     // 'jpeg' · 'png' · 'webp'
  let contactGridCols    = $state(4);
  let contactGridRows    = $state(6);
  let contactFrames      = $state(24);
  let frameExportMode    = $state('fps');      // 'fps' · 'interval'
  let frameExportFps     = $state(1);
  let frameExportInterval = $state(5);
  let frameExportFormat  = $state('jpeg');     // 'jpeg' · 'png' · 'webp'
  let watermarkPath      = $state(null);
  let watermarkCorner    = $state('br');       // 'tl' · 'tr' · 'bl' · 'br' · 'center'
  let watermarkOpacity   = $state(0.8);
  let watermarkScale     = $state(15);         // % of video width
  // ── Audio processing ───────────────────────────────────────────────────
  let channelToolsMode   = $state('stereo_to_mono'); // 'stereo_to_mono' · 'swap' · 'mute_l' · 'mute_r' · 'mono_to_stereo'
  let padSilenceHead     = $state(0);          // seconds
  let padSilenceTail     = $state(0);          // seconds
  // ── Chroma Key (FFmpeg tier) ───────────────────────────────────────────
  // Built-in chromakey/colorkey/hsvkey filters + optional despill. Writes
  // an alpha-capable container. A one-frame preview PNG renders in-panel.
  let chromaAlgo          = $state('chromakey');  // 'chromakey' · 'colorkey' · 'hsvkey'
  let chromaColor         = $state('#00ff00');    // HTML color picker (RGB hex)
  let chromaSimilarity    = $state(0.10);         // 0.01..0.40
  let chromaBlend         = $state(0.10);         // 0.0..0.5 (soft edge)
  let chromaDespill       = $state(true);         // toggle — ignored for colorkey
  let chromaDespillMix    = $state(0.50);         // 0.0..1.0
  let chromaUpsample      = $state(true);         // prepend format=yuv444p
  let chromaOutputFormat  = $state('mov_prores4444'); // alpha container
  let chromaPreviewUrl    = $state(null);         // asset:// URL of last preview
  let chromaPreviewLoading = $state(false);
  let chromaPreviewError  = $state(null);
  let _chromaPreviewTimer = null;
  let _chromaPreviewKey   = null;                 // last-rendered param hash
  // ── Analysis ops ───────────────────────────────────────────────────────
  // All seven analysis tools share the same pattern: inputs + Run + results panel.
  // Backend wiring lands in Phase 2; for now the Run buttons are inert stubs.
  // Loudness & TP
  let loudnessTarget = $state('-23');       // '-23' broadcast · '-16' streaming · '-14' Spotify · 'custom'
  let loudnessTargetCustom = $state(-23);
  let loudnessTruePeak = $state(true);      // peak=true flag (4× oversample, slower but accurate)
  let loudnessResult = $state(null);        // { I, LRA, TP, threshold } | null
  // Audio Norm
  let audioNormMode = $state('ebu');        // 'ebu' (two-pass loudnorm) · 'peak' · 'rg' (ReplayGain tag-only)
  let audioNormTargetI = $state(-16);
  let audioNormTargetTP = $state(-1.5);
  let audioNormTargetLRA = $state(11);
  let audioNormLinear = $state(true);       // linear=true preserves dynamics in two-pass
  // Cut Detection
  let cutDetectAlgo = $state('scdet');      // 'scdet' (FFmpeg ≥4.4) · 'scene'
  let cutDetectThreshold = $state(10);      // scdet: 5–15 · scene: 0.2–0.5 (scaled at runtime)
  let cutDetectMinShotS = $state(0.5);      // minimum shot length in seconds (post-filter)
  let cutDetectResults = $state([]);        // [{ time, score }]
  // Black Detection
  let blackDetectMinDur = $state(0.1);      // d= min black duration
  let blackDetectPixTh = $state(0.10);      // pix_th
  let blackDetectPicTh = $state(0.98);      // pic_th
  let blackDetectResults = $state([]);      // [{ start, end, duration }]
  // VMAF
  let vmafReferencePath = $state(null);     // reference (source-of-truth) video
  let vmafDistortedPath = $state(null);     // distorted (encoded) video
  let vmafModel = $state('hd');             // 'hd' (vmaf_v0.6.1) · '4k' (vmaf_4k_v0.6.1) · 'phone'
  let vmafSubsample = $state(1);            // n_subsample — 1 = every frame, 5 = every 5th
  let vmafResult = $state(null);            // { mean, min, max, harmonic_mean }
  // FrameMD5
  let frameMd5Stream = $state('video');     // 'video' · 'audio' · 'both'
  let frameMd5DiffPath = $state(null);      // optional second file for diff mode
  let frameMd5Result = $state(null);        // { hashes, firstDivergence? }
  // Subtitling · analyze tab extras
  let subLintCpsMax = $state(21);           // chars-per-second ceiling
  let subLintMinDurMs = $state(1000);       // <1s → warn
  let subLintMaxDurMs = $state(7000);       // >7s → warn
  let subLintLineMaxChars = $state(42);
  let subLintMaxLines = $state(2);
  let subDiffReferencePath = $state(null);  // optional reference subtitle for diff
  let subLintResults = $state(null);        // LintIssue[] | null
  let subDiffResults = $state(null);        // SubDiffLine[] | null
  let subProbeResults = $state(null);       // SubStream[] | null
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
  let selectedWidth       = $state(null);
  let selectedHeight      = $state(null);
  let previewLoading      = $state(false);
  let liveSrc             = $state(null);
  let tlMediaReady        = $state(false);
  let tlWaveformReady     = $state(false);
  let tlSpectrogramReady  = $state(false);
  let tlFilmstripReady    = $state(false);
  let _generation         = 0;

  // ── Pre-load cache ─────────────────────────────────────────────────────────
  // Loads waveform + filmstrip for queue items in the background, one at a time.
  // Priority slot: the existing pipeline serves items the user clicks immediately.
  let preloadCache = new Map(); // id → { waveform?, filmstripFrames? }
  let _bgBusy = false;
  let _unlistenBgFilmstrip = null;
  let cachedWaveformForTimeline = $state(null);
  let cachedFilmstripForTimeline = $state(null);

  function onPreviewLoaded() { previewLoading = false; }
  function onVideoMetaLoaded() {
    previewLoading = false;
    // Seek to first real frame — prevents "black preview" for videos that open on black
    if (videoEl) videoEl.currentTime = 0.001;
  }

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
        selectedWidth    = info.width         ?? null;
        selectedHeight   = info.height        ?? null;
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
    // Skip the delay if filmstrip is already cached (instant display).
    if (!cachedFilmstripForTimeline) {
      await frameYield(800);
    }
    if (stale()) return;
    tlFilmstripReady = true;
  }

  // File info dialog

  // Browse file input
  let fileInput = $state(null);

  // Preview video element — bound so Timeline can drive it
  let videoEl = $state(null);

  // Advanced audio panel — persists across file switches
  let vizExpanded = $state(false);
  let queueCompact = $state(false);
  let recurseSubfolders = $state(true);

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
        addFiles(files);
        imageOptions.output_dir = testDir;
        videoOptions.output_dir = testDir;
        audioOptions.output_dir = testDir;
        dataOptions.output_dir = testDir;
        documentOptions.output_dir = testDir;
        archiveOptions.output_dir = testDir;
      }
    } catch { /* non-fatal — folder may not exist */ }

    // Background filmstrip preload accumulator — listens for '-bg' suffixed events
    _unlistenBgFilmstrip = await listen('filmstrip-frame', (ev) => {
      const { id, index, data } = ev.payload;
      if (!id.endsWith('-bg')) return;
      const realId = id.slice(0, -3);
      const cached = preloadCache.get(realId);
      if (!cached) return;
      if (!cached.filmstripFrames) cached.filmstripFrames = new Array(20).fill(null);
      cached.filmstripFrames[index] = data;
      preloadCache.set(realId, cached);
    });

    unlistenProgress = await listen('job-progress', ({ payload }) => {
      const item = queue.find(q => q.id === payload.job_id);
      if (item) {
        item.status = 'converting';
        item.percent = payload.percent;
        setStatus(payload.message, 'info');
      }
    });

    unlistenDone = await listen('job-done', ({ payload }) => {
      const item = queue.find(q => q.id === payload.job_id);
      if (item) {
        item.status = 'done';
        item.percent = 100;
        item.outputPath = payload.output_path;
      }
      checkAllDone();
    });

    unlistenError = await listen('job-error', ({ payload }) => {
      const item = queue.find(q => q.id === payload.job_id);
      if (item) {
        item.status = 'error';
        item.error = payload.message;
      }
      pushError('job', `Conversion failed: ${item?.name ?? payload.job_id}`, payload.message);
      checkAllDone();
    });

    unlistenCancelled = await listen('job-cancelled', ({ payload }) => {
      const item = queue.find(q => q.id === payload.job_id);
      if (item) {
        item.status = 'cancelled';
        item.percent = 0;
      }
      checkAllDone();
    });

    presetManagerEl?.loadPresets();
    checkTools();

    // Fire-and-forget update check after a short delay so startup isn't blocked.
    setTimeout(() => { updateManagerEl?.maybeCheckForUpdate(); }, 2000);
  });

  onDestroy(() => {
    window.removeEventListener('keydown', zoom.handleKey);
    _unlistenBgFilmstrip?.();
    unlistenProgress?.();
    unlistenDone?.();
    unlistenError?.();
    unlistenCancelled?.();
  });

  // ── Background preloader ───────────────────────────────────────────────────
  // Processes queue items one at a time: waveform first (sequential), then fires
  // filmstrip in the background (events accumulate via _unlistenBgFilmstrip).
  // Priority slot: existing pipeline handles whichever item the user clicks.
  async function _bgPreloadNext() {
    if (_bgBusy) return;

    // Find the first uncached media item that isn't currently selected
    const nextItem = queue.find(item =>
      item.id !== selectedId &&
      ['video', 'audio'].includes(item.mediaType) &&
      !preloadCache.has(item.id)
    );
    if (!nextItem) return;

    _bgBusy = true;
    const cached = {};
    preloadCache.set(nextItem.id, cached); // mark as in-progress

    // Waveform (blocking — one at a time). Heavy audio *or* video flips draft
    // (e.g. a 3-hour high-fidelity recording — size or duration either trips).
    try {
      const data = await invoke('get_waveform', { path: nextItem.path, draft: isHeavyItem(nextItem) });
      cached.waveform = data;
      preloadCache.set(nextItem.id, cached);
    } catch { /* non-fatal */ }

    // Filmstrip (video only) — fire-and-forget; frames arrive via bg listener.
    // 20 frames either way; heavy flips to scale=30 for a ~75% per-frame cut.
    if (nextItem.mediaType === 'video') {
      const dur = nextItem.info?.duration_secs ?? null;
      if (dur) {
        const draft = isHeavyItem(nextItem);
        cached.filmstripFrames = new Array(20).fill(null);
        preloadCache.set(nextItem.id, cached);
        invoke('get_filmstrip', {
          path: nextItem.path, id: nextItem.id + '-bg',
          count: 20, duration: dur, draft
        }).catch(() => {});
      }
    }

    _bgBusy = false;
    setTimeout(_bgPreloadNext, 100); // chain to next item
  }

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

  function addFiles(paths) {
    for (const path of paths) {
      const name = path.split('/').pop() ?? path;
      const ext = name.includes('.') ? name.split('.').pop().toLowerCase() : '';
      const mt = mediaTypeFor(ext);
      const id = crypto.randomUUID();
      const item = { id, kind: 'file', parentId: null, path, name, ext, mediaType: mt, status: 'pending', percent: 0, info: null };
      queue.push(item);
      // Fetch file stats in the background for display in the queue
      if (['video', 'audio', 'image'].includes(mt)) {
        invoke('get_file_info', { path }).then(info => {
          const q = queue.find(q => q.id === id);
          if (q) q.info = info;
        }).catch(() => {});
      }
    }
    // Start background preloading for newly added items (small delay so queue renders first)
    setTimeout(_bgPreloadNext, 500);
  }

  function removeItem(id) {
    queue = queue.filter(q => q.id !== id);
    if (selectedId === id) handleSelect(queue.length > 0 ? queue[0].id : null);
  }

  // ── Batch folders ─────────────────────────────────────────────────────────
  // UI-only scaffolding for grouping files into a batch-output folder. Each
  // folder carries its own output options (rename rules, mirror structure,
  // in-place proxy). Wiring up the actual rendering pipeline is a follow-up.

  let batchFolderCounter = $state(0);

  function addBatchFolder() {
    batchFolderCounter += 1;
    const id = crypto.randomUUID();
    queue.push({
      id,
      kind: 'folder',
      name: `Proxy Node ${batchFolderCounter}`,
      expanded: true,
      status: 'pending',
      // Placeholder option shape — values are not wired to the render pipeline yet.
      batchOptions: {
        renameMode: 'suffix',          // 'suffix' | 'prefix' | 'pattern' | 'keep'
        renameToken: '_proxy',
        renamePattern: '{name}_{n}',
        outputMode: 'mirror',          // 'mirror' | 'inplace' | 'flat'
        outputRoot: '',
        preserveStructure: true,
      },
    });
    handleSelect(id);
  }

  /** Move a queued file into a batch folder (or to root when targetFolderId is null).
   *  Hard-noop on any unsafe shape — folders never nest, an item never targets
   *  itself, and a missing/non-folder target is silently ignored. */
  function moveItemToFolder(itemId, targetFolderId) {
    if (!itemId || itemId === targetFolderId) return;
    const idx = queue.findIndex(q => q.id === itemId);
    if (idx === -1) return;
    const item = queue[idx];
    if (item.kind !== 'file') return;             // folders cannot be moved
    if (targetFolderId) {
      const target = queue.find(q => q.id === targetFolderId);
      if (!target || target.kind !== 'folder') return;
    }
    if (item.parentId === targetFolderId) return; // already there
    item.parentId = targetFolderId;
    if (targetFolderId) {
      queue.splice(idx, 1);
      const fIdx = queue.findIndex(q => q.id === targetFolderId);
      queue.splice(fIdx + 1, 0, item);
    }
  }

  function toggleFolderExpanded(id) {
    const f = queue.find(q => q.id === id);
    if (f && f.kind === 'folder') f.expanded = !f.expanded;
  }

  function clearQueue() {
    queue = [];
    selectedId = null;
    selectedIds = new Set();
    selectAnchorId = null;
    setStatus('', 'info');
    converting = false;
    paused = false;
    validationErrors = {};
    batchIds = new Set();
  }

  // ── Browse ─────────────────────────────────────────────────────────────────

  function onBrowse() { fileInput?.click(); }

  function onFileInputChange(e) {
    const paths = Array.from(e.target.files ?? []).map(f => f.path ?? f.name);
    if (paths.length) addFiles(paths);
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

  // ── Operations: mechanical ops (rewrap, extract, cut, merge, replace-audio)
  // All share the same shape: set status, compute output path, invoke
  // `run_operation`, and let the generic job-progress / job-done listeners
  // handle UI updates (same as runConform below).

  async function runRewrap() {
    if (!selectedItem) { setStatus('Select a file first', 'error'); return; }
    if (selectedItem.status === 'converting') return;
    const outPath = expectedOutputPath(selectedItem, rewrapFormat, 'rewrap', outputDir, outputSeparator);
    selectedItem.status = 'converting';
    selectedItem.percent = 0;
    selectedItem.error = null;
    try {
      await invoke('run_operation', {
        jobId: selectedItem.id,
        operation: { type: 'rewrap', input_path: selectedItem.path, output_path: outPath },
      });
    } catch (err) {
      selectedItem.status = 'error';
      selectedItem.error = String(err);
      setStatus(`Rewrap failed: ${err}`, 'error');
    }
  }

  async function runCut() {
    if (!selectedItem) { setStatus('Select a file first', 'error'); return; }
    if (selectedItem.status === 'converting') return;
    const opts = selectedItem.mediaType === 'video' ? videoOptions : audioOptions;
    const outExt = selectedItem.ext || (selectedItem.mediaType === 'video' ? 'mp4' : 'wav');
    const outPath = expectedOutputPath(selectedItem, outExt, 'cut', outputDir, outputSeparator);
    selectedItem.status = 'converting';
    selectedItem.percent = 0;
    selectedItem.error = null;
    try {
      await invoke('run_operation', {
        jobId: selectedItem.id,
        operation: {
          type: 'cut',
          input_path: selectedItem.path,
          start_secs: opts.trim_start ?? null,
          end_secs: opts.trim_end ?? null,
          output_path: outPath,
        },
      });
    } catch (err) {
      selectedItem.status = 'error';
      selectedItem.error = String(err);
      setStatus(`Cut failed: ${err}`, 'error');
    }
  }

  async function runReplaceAudio() {
    if (!selectedItem || selectedItem.mediaType !== 'video') {
      setStatus('Select a video first', 'error'); return;
    }
    if (!replaceAudioPath) { setStatus('Pick a replacement audio file', 'error'); return; }
    if (selectedItem.status === 'converting') return;
    const outPath = expectedOutputPath(selectedItem, selectedItem.ext || 'mp4', 'replaced', outputDir, outputSeparator);
    selectedItem.status = 'converting';
    selectedItem.percent = 0;
    selectedItem.error = null;
    try {
      await invoke('run_operation', {
        jobId: selectedItem.id,
        operation: {
          type: 'replace_audio',
          video_path: selectedItem.path,
          audio_path: replaceAudioPath,
          output_path: outPath,
        },
      });
    } catch (err) {
      selectedItem.status = 'error';
      selectedItem.error = String(err);
      setStatus(`Replace audio failed: ${err}`, 'error');
    }
  }

  async function runMerge() {
    if (mergeSelection.length < 2) {
      setStatus('Add at least 2 files to merge', 'error'); return;
    }
    const items = mergeSelection
      .map(id => queue.find(q => q.id === id))
      .filter(Boolean);
    if (items.length < 2) { setStatus('Merge list has missing items', 'error'); return; }

    const first = items[0];
    const outExt = first.ext || 'mp4';
    const outPath = expectedOutputPath(first, outExt, 'merged', outputDir, outputSeparator);

    first.status = 'converting';
    first.percent = 0;
    first.error = null;
    try {
      await invoke('run_operation', {
        jobId: first.id,
        operation: {
          type: 'merge',
          input_paths: items.map(i => i.path),
          output_path: outPath,
        },
      });
    } catch (err) {
      first.status = 'error';
      first.error = String(err);
      setStatus(`Merge failed: ${err}`, 'error');
    }
  }

  async function runExtract() {
    if (!selectedItem) { setStatus('Select a file first', 'error'); return; }
    if (selectedItem.status === 'converting') return;
    let streams = [];
    try {
      streams = await invoke('get_streams', { inputPath: selectedItem.path });
    } catch (err) {
      setStatus(`Extract: probe failed: ${err}`, 'error');
      return;
    }
    // Filter streams by mode. For 'video' we take the first video stream
    // (one output). For 'audio' / 'subtitle' / 'all' we write one file per
    // matching stream — keeps the per-track UI deferred to a later pass.
    let targets = [];
    if (extractMode === 'video') {
      const v = streams.find(s => s.stream_type === 'video');
      if (v) targets = [v];
    } else if (extractMode === 'all') {
      targets = streams.filter(s => ['video', 'audio', 'subtitle'].includes(s.stream_type));
    } else {
      targets = streams.filter(s => s.stream_type === extractMode);
    }
    if (targets.length === 0) {
      setStatus(`No ${extractMode} streams found`, 'error'); return;
    }

    selectedItem.status = 'converting';
    selectedItem.percent = 0;
    selectedItem.error = null;

    for (const s of targets) {
      // Pick an output extension matching the stream type/codec.
      const ext = s.stream_type === 'video'
        ? (s.codec === 'h264' ? 'h264' : s.codec === 'hevc' ? 'hevc' : 'mkv')
        : s.stream_type === 'audio'
        ? (s.codec === 'aac' ? 'aac' : s.codec === 'mp3' ? 'mp3' : s.codec === 'opus' ? 'opus' : s.codec === 'flac' ? 'flac' : 'mka')
        : s.stream_type === 'subtitle'
        ? (s.codec === 'subrip' ? 'srt' : s.codec === 'ass' || s.codec === 'ssa' ? 'ass' : s.codec === 'webvtt' ? 'vtt' : 'mks')
        : 'bin';
      const suffix = targets.length > 1
        ? `extract_${s.stream_type}_${s.index}`
        : `extract_${s.stream_type}`;
      const outPath = expectedOutputPath(selectedItem, ext, suffix, outputDir, outputSeparator);
      // Each extract is its own job; last one reuses selectedItem.id so the UI
      // can reflect progress; earlier ones use a derived id so they don't
      // stomp the queue row (we still surface errors via status).
      const jobId = targets.length === 1 || s === targets[targets.length - 1]
        ? selectedItem.id
        : `${selectedItem.id}__extract_${s.index}`;
      try {
        await invoke('run_operation', {
          jobId,
          operation: {
            type: 'extract',
            input_path: selectedItem.path,
            stream_index: s.index,
            stream_type: s.stream_type,
            output_path: outPath,
          },
        });
      } catch (err) {
        setStatus(`Extract stream ${s.index} failed: ${err}`, 'error');
      }
    }
  }

  // ── Operations: run Conform on the selected item ──────────────────────────
  async function runConform() {
    if (!selectedItem || selectedItem.mediaType !== 'video') {
      setStatus('Select a video first', 'error');
      return;
    }
    if (selectedItem.status === 'converting') return;

    const outPath = expectedOutputPath(selectedItem, 'mp4', 'conformed', outputDir, outputSeparator);

    selectedItem.status = 'converting';
    selectedItem.percent = 0;
    selectedItem.error = null;

    try {
      await invoke('run_operation', {
        jobId: selectedItem.id,
        operation: {
          type: 'conform',
          input_path: selectedItem.path,
          output_path: outPath,
          fps: conformFps === 'source' ? null : conformFps,
          resolution: conformResolution === 'source' ? null : conformResolution,
          pix_fmt: conformPixFmt === 'source' ? null : conformPixFmt,
          fps_algo: conformFpsAlgo,   // 'drop' | 'blend' | 'mci'
          scale_algo: conformScaleAlgo, // 'bilinear' | 'bicubic' | 'lanczos' | 'spline'
          dither: conformDither,
        },
      });
    } catch (err) {
      selectedItem.status = 'error';
      selectedItem.error = String(err);
      setStatus(`Conform failed: ${err}`, 'error');
    }
  }

  // ── Analysis ops (read-only — no job pipeline) ────────────────────────────

  async function runLoudness() {
    if (!selectedItem) { setStatus('Select a file first', 'error'); return; }
    const target = loudnessTarget === 'custom'
      ? Number(loudnessTargetCustom)
      : Number(loudnessTarget);
    loudnessResult = null;
    setStatus('Measuring loudness…', 'info');
    try {
      loudnessResult = await invoke('analyze_loudness', {
        inputPath: selectedItem.path,
        targetI: target,
        targetTp: -2.0,
        truePeak: loudnessTruePeak,
      });
      setStatus('Loudness measured', 'success');
    } catch (err) {
      setStatus(`Loudness failed: ${err}`, 'error');
    }
  }

  async function runCutDetect() {
    if (!selectedItem) { setStatus('Select a file first', 'error'); return; }
    cutDetectResults = [];
    setStatus('Detecting cuts…', 'info');
    try {
      const res = await invoke('analyze_cut_detect', {
        inputPath: selectedItem.path,
        algo: cutDetectAlgo,
        threshold: Number(cutDetectThreshold),
        minShotS: Number(cutDetectMinShotS),
      });
      cutDetectResults = res.map(c => ({
        time: c.time.toFixed(3),
        score: c.score.toFixed(2),
      }));
      setStatus(`${cutDetectResults.length} cuts`, 'success');
    } catch (err) {
      setStatus(`Cut detect failed: ${err}`, 'error');
    }
  }

  async function runBlackDetect() {
    if (!selectedItem) { setStatus('Select a file first', 'error'); return; }
    blackDetectResults = [];
    setStatus('Detecting black frames…', 'info');
    try {
      const res = await invoke('analyze_black_detect', {
        inputPath: selectedItem.path,
        minDuration: Number(blackDetectMinDur),
        pixTh: Number(blackDetectPixTh),
        picTh: Number(blackDetectPicTh),
      });
      blackDetectResults = res.map(b => ({
        start: b.start.toFixed(3),
        end: b.end.toFixed(3),
        duration: b.duration.toFixed(3),
      }));
      setStatus(`${blackDetectResults.length} black intervals`, 'success');
    } catch (err) {
      setStatus(`Black detect failed: ${err}`, 'error');
    }
  }

  async function runVmaf() {
    if (!vmafReferencePath || !vmafDistortedPath) return;
    vmafResult = null;
    setStatus('Computing VMAF…', 'info');
    try {
      const r = await invoke('analyze_vmaf', {
        referencePath: vmafReferencePath,
        distortedPath: vmafDistortedPath,
        model: vmafModel,
        subsample: Number(vmafSubsample),
      });
      vmafResult = {
        mean: r.mean.toFixed(2),
        min: r.min.toFixed(2),
        max: r.max.toFixed(2),
        harmonic_mean: r.harmonic_mean.toFixed(2),
      };
      setStatus('VMAF complete', 'success');
    } catch (err) {
      setStatus(`VMAF failed: ${err}`, 'error');
    }
  }

  async function runFrameMd5() {
    if (!selectedItem) { setStatus('Select a file first', 'error'); return; }
    frameMd5Result = null;
    setStatus('Hashing frames…', 'info');
    try {
      const r = await invoke('analyze_framemd5', {
        inputPath: selectedItem.path,
        stream: frameMd5Stream,
        diffPath: frameMd5DiffPath,
      });
      frameMd5Result = {
        hashes: r.hashes,
        firstDivergence: r.first_divergence,
      };
      setStatus('FrameMD5 complete', 'success');
    } catch (err) {
      setStatus(`FrameMD5 failed: ${err}`, 'error');
    }
  }

  async function runAudioNorm() {
    if (!selectedItem) { setStatus('Select a file first', 'error'); return; }
    if (selectedItem.status === 'converting') return;
    const outExt = selectedItem.ext || (selectedItem.mediaType === 'video' ? 'mp4' : 'wav');
    const outPath = expectedOutputPath(selectedItem, outExt, 'normalized', outputDir, outputSeparator);
    selectedItem.status = 'converting';
    selectedItem.percent = 0;
    selectedItem.error = null;
    try {
      await invoke('run_operation', {
        jobId: selectedItem.id,
        operation: {
          type: 'audio_normalize',
          input_path: selectedItem.path,
          output_path: outPath,
          mode: audioNormMode,                  // 'ebu' | 'peak' | 'rg'
          target_i: Number(audioNormTargetI),
          target_tp: Number(audioNormTargetTP),
          target_lra: Number(audioNormTargetLRA),
          linear: audioNormLinear,
        },
      });
    } catch (err) {
      selectedItem.status = 'error';
      selectedItem.error = String(err);
      setStatus(`Audio normalize failed: ${err}`, 'error');
    }
  }

  async function runSilenceRemove() {
    if (!selectedItem) { setStatus('Select a file first', 'error'); return; }
    if (selectedItem.status === 'converting') return;
    const outExt = selectedItem.ext || (selectedItem.mediaType === 'video' ? 'mp4' : 'wav');
    const outPath = expectedOutputPath(selectedItem, outExt, 'unsilenced', outputDir, outputSeparator);
    selectedItem.status = 'converting';
    selectedItem.percent = 0;
    selectedItem.error = null;
    try {
      await invoke('run_operation', {
        jobId: selectedItem.id,
        operation: {
          type: 'silence_remove',
          input_path: selectedItem.path,
          output_path: outPath,
          threshold_db: Number(silenceThresholdDb),
          min_silence_s: Number(silenceMinDurS),
          pad_ms: Number(silencePadMs),
        },
      });
    } catch (err) {
      selectedItem.status = 'error';
      selectedItem.error = String(err);
      setStatus(`Silence remove failed: ${err}`, 'error');
    }
  }

  // ── Generic op runner — wraps boilerplate around run_operation invoke ────
  async function _runOp({ payload, suffix, outExt, label, requireVideo = false }) {
    if (!selectedItem) { setStatus('Select a file first', 'error'); return; }
    if (selectedItem.status === 'converting') return;
    if (requireVideo && selectedItem.mediaType !== 'video') {
      setStatus(`${label}: video file required`, 'error');
      return;
    }
    const ext = outExt || selectedItem.ext || (selectedItem.mediaType === 'video' ? 'mp4' : 'wav');
    const outPath = expectedOutputPath(selectedItem, ext, suffix, outputDir, outputSeparator);
    selectedItem.status = 'converting';
    selectedItem.percent = 0;
    selectedItem.error = null;
    try {
      await invoke('run_operation', {
        jobId: selectedItem.id,
        operation: { ...payload, input_path: selectedItem.path, output_path: outPath },
      });
    } catch (err) {
      selectedItem.status = 'error';
      selectedItem.error = String(err);
      setStatus(`${label} failed: ${err}`, 'error');
    }
  }

  async function runRemoveAudio() {
    const outExt = selectedItem?.ext || 'mp4';
    return _runOp({ payload: { type: 'remove_audio' }, suffix: 'noaudio', outExt, label: 'Remove audio' });
  }

  async function runRemoveVideo() {
    // Pick an audio container that accepts stream-copied audio of any codec.
    const outExt = 'mka';
    return _runOp({ payload: { type: 'remove_video' }, suffix: 'audio-only', outExt, label: 'Remove video' });
  }

  async function runMetadataStrip() {
    return _runOp({
      payload: {
        type: 'metadata_strip',
        mode: metadataStripMode,
        title_value: metadataStripMode === 'title' ? String(metadataStripTitle || '') : null,
      },
      suffix: 'stripped',
      label: 'Metadata strip',
    });
  }

  async function runLoop() {
    const n = Math.max(2, Math.min(50, Number(loopCount) || 2));
    return _runOp({
      payload: { type: 'loop', count: n },
      suffix: `x${n}`,
      label: 'Loop',
    });
  }

  async function runRotateFlip() {
    return _runOp({
      payload: { type: 'rotate_flip', mode: rotateFlipMode },
      suffix: 'rotated',
      outExt: 'mp4',
      label: 'Rotate/Flip',
      requireVideo: true,
    });
  }

  async function runReverse() {
    return _runOp({
      payload: { type: 'reverse' },
      suffix: 'reversed',
      outExt: 'mp4',
      label: 'Reverse',
    });
  }

  async function runSpeed() {
    const rate = speedPreset === 'custom'
      ? Math.max(0.1, Math.min(10.0, Number(speedCustom) || 1.0))
      : Number(speedPreset);
    return _runOp({
      payload: { type: 'speed', rate },
      suffix: `${rate}x`,
      outExt: 'mp4',
      label: 'Speed',
    });
  }

  async function runFade() {
    const fi = Math.max(0, Math.min(10, Number(fadeInS) || 0));
    const fo = Math.max(0, Math.min(10, Number(fadeOutS) || 0));
    if (fi === 0 && fo === 0) { setStatus('Set at least one fade value', 'error'); return; }
    return _runOp({
      payload: { type: 'fade', fade_in: fi, fade_out: fo },
      suffix: 'faded',
      outExt: 'mp4',
      label: 'Fade',
    });
  }

  async function runDeinterlace() {
    return _runOp({
      payload: { type: 'deinterlace', mode: deinterlaceMode },
      suffix: 'deint',
      outExt: 'mp4',
      label: 'Deinterlace',
      requireVideo: true,
    });
  }

  async function runDenoise() {
    return _runOp({
      payload: { type: 'denoise', preset: denoisePreset },
      suffix: 'denoised',
      outExt: 'mp4',
      label: 'Denoise',
      requireVideo: true,
    });
  }

  async function runThumbnail() {
    const ext = thumbnailFormat === 'jpeg' ? 'jpg' : thumbnailFormat;
    const base = selectedItem?.path || '';
    const lastSlash = base.lastIndexOf('/');
    const parentDir = lastSlash >= 0 ? base.slice(0, lastSlash) : '.';
    const stem = selectedItem?.ext
      ? selectedItem.name.slice(0, -(selectedItem.ext.length + 1))
      : (selectedItem?.name || 'frame');
    const dir = outputDir ?? parentDir;
    const outPath = `${dir}/${stem}${outputSeparator}thumb.${ext}`;
    if (!selectedItem) { setStatus('Select a file first', 'error'); return; }
    selectedItem.status = 'converting'; selectedItem.percent = 0; selectedItem.error = null;
    try {
      await invoke('run_operation', {
        jobId: selectedItem.id,
        operation: {
          type: 'thumbnail',
          input_path: selectedItem.path,
          output_path: outPath,
          time_spec: String(thumbnailTime || '0'),
          format: thumbnailFormat,
        },
      });
    } catch (err) {
      selectedItem.status = 'error'; selectedItem.error = String(err);
      setStatus(`Thumbnail failed: ${err}`, 'error');
    }
  }

  async function runContactSheet() {
    const c = Math.max(1, Math.min(20, Number(contactGridCols) || 4));
    const r = Math.max(1, Math.min(20, Number(contactGridRows) || 6));
    const n = Math.max(c * r, Math.min(500, Number(contactFrames) || c * r));
    return _runOp({
      payload: { type: 'contact_sheet', cols: c, rows: r, frames: n },
      suffix: 'sheet',
      outExt: 'png',
      label: 'Contact sheet',
      requireVideo: true,
    });
  }

  async function runFrameExport() {
    if (!selectedItem) { setStatus('Select a file first', 'error'); return; }
    const base = selectedItem.path;
    const lastSlash = base.lastIndexOf('/');
    const parentDir = lastSlash >= 0 ? base.slice(0, lastSlash) : '.';
    const stem = selectedItem.ext
      ? selectedItem.name.slice(0, -(selectedItem.ext.length + 1))
      : selectedItem.name;
    const dir = outputDir ?? parentDir;
    const outDir = `${dir}/${stem}_frames`;
    const value = frameExportMode === 'fps'
      ? Math.max(0.01, Number(frameExportFps) || 1)
      : Math.max(0.01, Number(frameExportInterval) || 5);
    selectedItem.status = 'converting'; selectedItem.percent = 0; selectedItem.error = null;
    try {
      await invoke('run_operation', {
        jobId: selectedItem.id,
        operation: {
          type: 'frame_export',
          input_path: selectedItem.path,
          output_dir: outDir,
          mode: frameExportMode,
          value,
          format: frameExportFormat,
        },
      });
    } catch (err) {
      selectedItem.status = 'error'; selectedItem.error = String(err);
      setStatus(`Frame export failed: ${err}`, 'error');
    }
  }

  async function runWatermark() {
    if (!watermarkPath) { setStatus('Pick a watermark PNG first', 'error'); return; }
    return _runOp({
      payload: {
        type: 'watermark',
        watermark_path: watermarkPath,
        corner: watermarkCorner,
        opacity: Number(watermarkOpacity),
        scale_pct: Number(watermarkScale),
      },
      suffix: 'watermarked',
      outExt: 'mp4',
      label: 'Watermark',
      requireVideo: true,
    });
  }

  async function runChannelTools() {
    return _runOp({
      payload: { type: 'channel_tools', mode: channelToolsMode },
      suffix: 'ch-' + channelToolsMode,
      label: 'Channel tools',
    });
  }

  async function runPadSilence() {
    const h = Math.max(0, Math.min(60, Number(padSilenceHead) || 0));
    const t = Math.max(0, Math.min(60, Number(padSilenceTail) || 0));
    if (h === 0 && t === 0) { setStatus('Set at least one pad value', 'error'); return; }
    return _runOp({
      payload: { type: 'pad_silence', head_s: h, tail_s: t },
      suffix: 'padded',
      label: 'Pad silence',
    });
  }

  // ── Chroma Key (FFmpeg tier) ──────────────────────────────────────────────
  // Segmented output selector → (ext, rust target id, suffix). PNG sequence
  // writes a sibling `<stem>_frames/` directory; ext stays empty in that case.
  function _chromaOutputMeta() {
    switch (chromaOutputFormat) {
      case 'mov_qtrle':        return { ext: 'mov', target: 'mov_qtrle',       suffix: 'keyed' };
      case 'mov_prores4444':   return { ext: 'mov', target: 'mov_prores4444',  suffix: 'keyed' };
      case 'webm_vp9':         return { ext: 'webm',target: 'webm_vp9',        suffix: 'keyed' };
      case 'mkv_ffv1':         return { ext: 'mkv', target: 'mkv_ffv1',        suffix: 'keyed' };
      case 'png_sequence':     return { ext: '',    target: 'png_sequence',    suffix: 'frames' };
      default:                 return { ext: 'mov', target: 'mov_prores4444',  suffix: 'keyed' };
    }
  }

  async function runChromaKey() {
    if (!selectedItem) { setStatus('Select a file first', 'error'); return; }
    if (selectedItem.mediaType !== 'video') { setStatus('Chroma key: video file required', 'error'); return; }
    if (selectedItem.status === 'converting') return;

    const meta = _chromaOutputMeta();
    // PNG sequence produces a sibling directory rather than a single file.
    // We reuse expectedOutputPath with a placeholder extension then strip it.
    let outPath;
    if (meta.target === 'png_sequence') {
      const base = expectedOutputPath(selectedItem, 'x', meta.suffix, outputDir, outputSeparator);
      outPath = base.replace(/\.x$/, '');
    } else {
      outPath = expectedOutputPath(selectedItem, meta.ext, meta.suffix, outputDir, outputSeparator);
    }

    selectedItem.status = 'converting';
    selectedItem.percent = 0;
    selectedItem.error = null;
    try {
      await invoke('run_operation', {
        jobId: selectedItem.id,
        operation: {
          type: 'chroma_key',
          input_path: selectedItem.path,
          output_path: outPath,
          algo: chromaAlgo,
          color_hex: String(chromaColor),
          similarity: Number(chromaSimilarity),
          blend: Number(chromaBlend),
          despill: !!chromaDespill,
          despill_mix: Number(chromaDespillMix),
          upsample: !!chromaUpsample,
          output_target: meta.target,
          trim_start: videoOptions.trim_start ?? null,
          trim_end:   videoOptions.trim_end   ?? null,
        },
      });
    } catch (err) {
      selectedItem.status = 'error';
      selectedItem.error = String(err);
      setStatus(`Chroma key failed: ${err}`, 'error');
    }
  }

  // Debounced single-frame preview. Hashes params so rapid slider moves
  // collapse into one ffmpeg call; cache by hash to avoid re-running when
  // nothing changed.
  function _chromaPreviewKeyOf() {
    return [
      selectedItem?.path, chromaAlgo, chromaColor,
      chromaSimilarity, chromaBlend,
      chromaDespill, chromaDespillMix, chromaUpsample,
      videoOptions.trim_start ?? 0,
    ].join('|');
  }

  async function generateChromaPreview() {
    if (!selectedItem || selectedItem.mediaType !== 'video') return;
    const key = _chromaPreviewKeyOf();
    if (key === _chromaPreviewKey && chromaPreviewUrl) return;
    if (_chromaPreviewTimer) clearTimeout(_chromaPreviewTimer);
    _chromaPreviewTimer = setTimeout(async () => {
      chromaPreviewLoading = true;
      chromaPreviewError = null;
      try {
        const t = Number(videoOptions.trim_start) > 0
          ? Number(videoOptions.trim_start)
          : 1.0;
        const path = await invoke('chroma_key_preview', {
          inputPath: selectedItem.path,
          timeS: t,
          algo: chromaAlgo,
          colorHex: String(chromaColor),
          similarity: Number(chromaSimilarity),
          blend: Number(chromaBlend),
          despill: !!chromaDespill,
          despillMix: Number(chromaDespillMix),
          upsample: !!chromaUpsample,
        });
        // Cache-bust so the <img> refreshes when the same path is rewritten.
        chromaPreviewUrl = convertFileSrc(path) + '?t=' + Date.now();
        _chromaPreviewKey = key;
      } catch (err) {
        chromaPreviewError = String(err);
      } finally {
        chromaPreviewLoading = false;
      }
    }, 250);
  }

  // ── Subtitling · analyze tab ──────────────────────────────────────────────

  async function runSubLint() {
    if (!selectedItem) { setStatus('Select a subtitle file first', 'error'); return; }
    subLintResults = null;
    try {
      subLintResults = await invoke('lint_subtitle', {
        inputPath: selectedItem.path,
        thresholds: {
          cps_max: Number(subLintCpsMax),
          min_dur_ms: Number(subLintMinDurMs),
          max_dur_ms: Number(subLintMaxDurMs),
          line_max_chars: Number(subLintLineMaxChars),
          max_lines: Number(subLintMaxLines),
        },
      });
      setStatus(`${subLintResults.length} lint issues`, 'success');
    } catch (err) {
      setStatus(`Subtitle lint failed: ${err}`, 'error');
    }
  }

  async function runSubDiff() {
    if (!selectedItem || !subDiffReferencePath) return;
    subDiffResults = null;
    try {
      subDiffResults = await invoke('diff_subtitle', {
        aPath: selectedItem.path,
        bPath: subDiffReferencePath,
      });
      setStatus('Subtitle diff complete', 'success');
    } catch (err) {
      setStatus(`Subtitle diff failed: ${err}`, 'error');
    }
  }

  async function runSubProbe() {
    if (!selectedItem) { setStatus('Select a video first', 'error'); return; }
    subProbeResults = null;
    try {
      subProbeResults = await invoke('probe_subtitles', { inputPath: selectedItem.path });
      setStatus(`${subProbeResults.length} subtitle streams`, 'success');
    } catch (err) {
      setStatus(`Subtitle probe failed: ${err}`, 'error');
    }
  }

  async function startConvert(mode = 'all') {
    const errors = validateOptions(videoOptions, audioOptions);
    if (Object.keys(errors).length > 0) { validationErrors = errors; return; }
    validationErrors = {};

    if (!globalOutputFormat) {
      setStatus('Select an output format first', 'error');
      return;
    }

    const candidates = mode === 'selected'
      ? (selectedItem ? [selectedItem] : [])
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

    for (const item of willRun) {
      if (paused) break;
      item.status = 'converting';
      item.percent = 0;

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

      invoke('convert_file', { jobId: item.id, inputPath: item.path, options: opts })
        .catch(err => { item.status = 'error'; item.error = String(err); checkAllDone(); });
    }
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
    if (settings.hideConverted && outputSuffix) {
      const suf = `${outputSeparator}${outputSuffix}`;
      out = out.filter(q => {
        const stem = q.ext ? q.name.slice(0, -(q.ext.length + 1)) : q.name;
        return !stem.endsWith(suf);
      });
    }
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
    if (expanded.length) addFiles(expanded);
  }

  // ── Presets ────────────────────────────────────────────────────────────────
  // Owned by PresetManager; loadPresets() is called via bind:this after mount.
  let presetManagerEl = $state(null);

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
      { id: 'usd',   label: 'USD',       todo: true, preview: true },
      { id: 'usdz',  label: 'USDZ',      todo: true, preview: true },
      { id: 'abc',   label: 'Alembic',   todo: true, preview: true },
      { id: 'blend', label: 'Blender',   todo: true, preview: true },
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
  let compatibleOutputCats = $derived(
    selectedItem ? (OUTPUT_CATS_FOR[selectedItem.mediaType] ?? null) : null
  );

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
      handleSelect(null);
    }
  });

  // ── Selection handler ──────────────────────────────────────────────────────
  // All mutations happen synchronously in the same call so Svelte batches them
  // into ONE DOM flush — guaranteed clear before any file I/O begins.
  // Then the async pipeline runs each loading stage in sequence.
  function deselectAll() {
    selectedIds = new Set();
    selectAnchorId = null;
    handleSelect(null);
  }

  /** Items the user is allowed to select (skips folders' incompatible status — folders always selectable). */
  function _selectableIds() {
    return visibleQueue
      .filter(q => q.kind === 'folder' || compatibleTypes.length === 0 || compatibleTypes.includes(q.mediaType))
      .map(q => q.id);
  }

  function handleSelect(id, mods = null) {
    settingsOpen = false;

    // Modifier-aware multi-select. Updates selectedIds + anchor, then falls
    // through to the existing single-selection pipeline using the most-recent id.
    if (mods && id) {
      if (mods.shift && selectAnchorId) {
        const order = _selectableIds();
        const a = order.indexOf(selectAnchorId);
        const b = order.indexOf(id);
        if (a !== -1 && b !== -1) {
          const [lo, hi] = a < b ? [a, b] : [b, a];
          const range = order.slice(lo, hi + 1);
          selectedIds = new Set(range);
        } else {
          selectedIds = new Set([id]);
          selectAnchorId = id;
        }
      } else if (mods.meta || mods.ctrl) {
        const next = new Set(selectedIds);
        if (next.has(id)) next.delete(id); else next.add(id);
        selectedIds = next;
        selectAnchorId = id;
      } else {
        selectedIds = new Set([id]);
        selectAnchorId = id;
      }
    } else if (id) {
      selectedIds = new Set([id]);
      selectAnchorId = id;
    } else {
      selectedIds = new Set();
      selectAnchorId = null;
    }

    const gen = ++_generation;  // cancel any in-flight pipeline
    const newItem = id ? queue.find(q => q.id === id) : null;
    const isMedia = !!(newItem && ['video', 'audio', 'image'].includes(newItem.mediaType));

    // ── Synchronous batch: clears everything in one Svelte flush ──
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
    selectedId         = id ?? null;

    // Auto-expand viz based on setting — only ever set to true, never force-collapse.
    // User's manual expand/collapse state is preserved across file switches.
    const vd = settings.vizDefault;
    const shouldExpand = newItem?.mediaType === 'video'
      ? vd === 'av'
      : newItem?.mediaType === 'audio'
        ? vd === 'audio' || vd === 'av'
        : false;
    if (shouldExpand) vizExpanded = true;

    // Serve pre-loaded data to Timeline immediately (avoids re-invoking ffmpeg)
    const _cached = preloadCache.get(id ?? '');
    cachedWaveformForTimeline = _cached?.waveform ?? null;
    cachedFilmstripForTimeline = _cached?.filmstripFrames ?? null;

    // ── Async pipeline: stages run sequentially after browser paints ──
    runLoadPipeline(gen, newItem);
  }

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

    <!-- ── LEFT: File queue (390px expanded / 234px compact) ──────────────── -->
    <aside class="{queueCompact ? 'w-[273px]' : 'w-[320px]'} shrink-0 border-r border-[var(--border)] flex flex-col bg-[var(--surface-raised)] relative {settingsOpen ? 'z-[500]' : 'z-50'}"
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
          onclick={addBatchFolder}
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
           style="background:var(--surface-raised)">
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
            placeholder="Filter files — try mp3, mov, name…"
            class="w-full pl-7 pr-6 py-1 text-[11px] rounded border border-[var(--border)]
                   bg-[color:color-mix(in_srgb,#000_35%,var(--surface-raised))]
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

      <!-- File list -->
      <Queue
        queue={visibleQueue}
        {selectedId}
        {selectedIds}
        onselect={handleSelect}
        onremove={(id) => removeItem(id)}
        oncancel={(id) => cancelJob(id)}
        ontogglefolder={toggleFolderExpanded}
        onmovetofolder={moveItemToFolder}
        ondragstartfile={(id) => draggingFileId = id}
        ondragendfile={() => draggingFileId = null}
        disableHoverInfo={selectedItem?.kind === 'folder'}
        compatibleTypes={compatibleTypes}
        compact={queueCompact}
        showExtColumn={settings.fileTypeColumn}
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
      <div class="shrink-0 border-t border-[var(--border)] flex items-center justify-center gap-1.5 px-3 py-1.5"
           style="background:var(--surface-raised)">
        <button
          onclick={clearQueue}
          disabled={queue.length === 0}
          class="btn-bevel px-2 py-0.5 text-[11px] shrink-0"
        >Clear</button>
        <button
          onclick={onBrowse}
          class="btn-bevel px-2 py-0.5 text-[11px] shrink-0"
        >Add Files</button>
        <button
          onclick={() => recurseSubfolders = !recurseSubfolders}
          data-tooltip="Recurse into subfolders when a directory is added"
          class="btn-bevel px-2 py-0.5 text-[11px] shrink-0 {recurseSubfolders ? 'is-active' : ''}"
        >Subfolders</button>
        <button
          onclick={deselectAll}
          disabled={selectedIds.size === 0}
          class="btn-bevel px-2 py-0.5 text-[11px] shrink-0"
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
            <div class="flex gap-1.5 pointer-events-none opacity-40">
              <div class="flex-1 py-1.5 rounded text-[12px] font-medium text-center border border-red-900/60 text-red-700">Convert Selected</div>
              <div class="flex-1 py-1.5 rounded text-[12px] font-semibold text-center bg-red-950/50 text-red-700">Convert All</div>
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
              class="flex items-center justify-center px-2 py-1 rounded text-[11px] border transition-colors flex-1 min-w-0
                     {outputDestMode === 'source'
                       ? 'bg-[var(--accent)] text-white border-[color-mix(in_srgb,var(--accent)_70%,#000)]'
                       : 'border-[var(--border)] text-[var(--text-secondary)] hover:text-[var(--text-primary)] hover:border-[var(--accent)]'}"
            >
              Source
            </button>
            <button
              onclick={() => { outputDestMode = 'custom'; folderInput?.click(); }}
              title={customOutputDir ? `Output → ${customOutputDir}` : 'Pick an output folder'}
              class="flex items-center justify-center px-2 py-1 rounded text-[11px] border transition-colors flex-1 min-w-0
                     {outputDestMode === 'custom'
                       ? 'bg-[var(--accent)] text-white border-[color-mix(in_srgb,var(--accent)_70%,#000)]'
                       : 'border-[var(--border)] text-[var(--text-secondary)] hover:text-[var(--text-primary)] hover:border-[var(--accent)]'}"
            >
              Browse
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
                <!-- Hide converted files toggle -->
                <label class="flex items-center justify-between gap-2 cursor-pointer">
                  <span class="text-[12px] text-[var(--text-primary)]">Hide converted files</span>
                  <input type="checkbox" bind:checked={settings.hideConverted}
                         class="w-3.5 h-3.5 accent-[var(--accent)]" />
                </label>
              </div>

              <!-- Section: UI -->
              <div class="px-4 pt-3 pb-3 border-b border-[var(--border)] flex flex-col gap-2.5">
                <p class="text-[10px] font-semibold uppercase tracking-widest text-[var(--text-secondary)]">UI</p>
                <!-- Visualizer default -->
                <div class="flex items-center justify-between gap-2">
                  <span class="text-[12px] text-[var(--text-primary)]">Visualizer</span>
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
              </div>

              <!-- Section: Data -->
              <div class="px-4 pt-3 pb-4 flex items-center justify-between gap-2">
                <div class="flex items-center justify-between w-full">
                  <!-- svelte-ignore a11y_missing_attribute -->
                  <a onclick={() => { settingsOpen = false; aboutOpen = true; }}
                     class="text-[11px] text-blue-400 underline decoration-blue-400/50 hover:decoration-blue-400 cursor-pointer transition-all select-none">
                    About
                  </a>
                  <span class="text-[10px] text-[var(--text-secondary)]/60 select-none">Fade{appVersion ? ` v${appVersion}` : ''}</span>
                </div>
                <button
                  onclick={() => { preloadCache.clear(); cachedWaveformForTimeline = null; cachedFilmstripForTimeline = null; }}
                  class="px-2.5 py-1 rounded text-[11px] border border-[var(--border)]
                         text-[var(--text-secondary)] hover:text-[var(--text-primary)] hover:border-[var(--accent)]
                         transition-colors">
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

          <!-- Status box: last job/queue outcome, right-justified.
               Colour coded: gray = info, green = success, red = error/warning. -->
          <div class="flex-1 min-w-0 px-2.5 flex items-center justify-end rounded
                      bg-[color-mix(in_srgb,#fff_8%,var(--surface-raised))]" aria-live="polite">
            <span class="text-[11px] truncate text-right
                         {statusKind === 'success' ? 'text-green-400'
                          : statusKind === 'error' ? 'text-red-400'
                          : 'text-gray-400'}">{statusMessage}</span>
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
          {@const op = OPERATIONS.find(o => o.id === selectedOperation)}
          <div class="absolute inset-0 flex flex-col items-center justify-end p-8 overflow-y-auto gap-6 {selectedItem?.mediaType === 'video' ? 'pb-[340px]' : ''}">
            <div class="flex items-center gap-3 justify-center w-[560px]">
              <button
                onclick={exitOperationsMode}
                class="px-3 py-1.5 rounded text-[11px] font-medium border border-[var(--border)]
                       text-white hover:border-[var(--accent)] hover:bg-white/5 transition-colors shrink-0"
              >← Back</button>
              <h2 class="text-[20px] font-semibold text-white/85">{op?.label}</h2>
            </div>

            {#if selectedItem?.mediaType !== 'video'}
              <div class="w-[560px] aspect-video rounded-lg border border-dashed border-white/10
                          flex items-center justify-center text-[10px] text-white/25 bg-black/30 shrink-0">
                Video preview
              </div>
            {/if}

            <div class="rounded-lg border border-white/10 bg-white/[0.04] px-5 py-4 w-[560px] shrink-0">
              <p class="text-[11px] uppercase tracking-wider text-white/35 font-semibold mb-2">How it works</p>
              <p class="text-[13px] text-white/60 leading-relaxed">
                {#if selectedOperation === 'cut-noenc'}
                  {#if cutMode === 'cut'}
                    <strong class="text-white/80">Cut</strong> — keep the range between the trim handles and discard the rest. Stream copy, no re-encoding. Cuts snap to the nearest keyframe.
                  {:else}
                    <strong class="text-white/80">Extract</strong> — remove the range between the trim handles and glue the two halves back together. Requires re-encoding because the join point would otherwise break the GOP.
                  {/if}
                  <br/><br/><span class="text-white/35">Drag the trim handles on the timeline to set the range. Flip between Cut / Extract with the toggle below.</span>
                {:else if selectedOperation === 'replace-audio'}
                  Swap the audio track in a video with a new audio file. Video is copied untouched; only the audio is remuxed. If the new audio codec is incompatible with the container, it's transcoded to AAC automatically.
                  <br/><br/><span class="text-white/35">Select the video in the queue. Drag a new audio file as the replacement source.</span>
                {:else if selectedOperation === 'rewrap'}
                  Change the container format without re-encoding any streams — fast and lossless. Example: MKV → MP4. Codec compatibility with the target container is checked first; incompatible streams are flagged before the job starts.
                  <br/><br/><span class="text-white/35">Select a file from the queue and choose an output container.</span>
                {:else if selectedOperation === 'conform'}
                  Match a video to a reference specification: frame rate, resolution, and pixel format. Always requires re-encoding. Use fps=24000/1001 for broadcast 23.976 or fps=30000/1001 for 29.97.
                  <br/><br/><span class="text-white/35">Set target fps, resolution, and pixel format in the controls below.</span>
                {:else if selectedOperation === 'merge'}
                  Concatenate multiple video files into one in order. Files with matching codec, resolution, fps, and pixel format merge without re-encoding via the concat demuxer. Mismatched files are re-encoded to H.264/AAC automatically.
                  <br/><br/><span class="text-white/35">Add files to the queue in order — they will be joined top to bottom.</span>
                {:else if selectedOperation === 'extract'}
                  Pull individual streams out of a container: video-only, a specific audio track by index, or a subtitle track. Each stream is written to its own output file with no re-encoding.
                  <br/><br/><span class="text-white/35">Select a file, inspect its streams in the panel, and choose what to extract.</span>
                {:else if selectedOperation === 'subtitling'}
                  Two modes: burn-in (hard subs — subtitles are rendered into the video pixels, always re-encodes) or embed (soft subs — subtitle file is added as a selectable track, video is not re-encoded). SRT, ASS, VTT, and SSA formats supported.
                  <br/><br/><span class="text-white/35">Drag a subtitle file alongside the video. Choose burn-in or embed mode.</span>
                {:else if selectedOperation === 'video-inserts'}
                  Insert a video clip at a specific timecode in the main video. The source is split at the insert point, the clip is placed between the two halves, and everything is rejoined. Stream copy is used when all clips share the same codec and specs.
                  <br/><br/><span class="text-white/35">Mark the insert point on the timeline, then drag the insert clip into position.</span>
                {:else if selectedOperation === 'silence-remove'}
                  Detect silent gaps and cut them out. Uses ffmpeg <strong class="text-white/80">silenceremove</strong>. Threshold sets the dBFS floor; anything quieter than that for at least the minimum duration is removed. A padding value keeps a bit of silence around each speech region so onsets don't clip.
                  <br/><br/><span class="text-white/35">Tune threshold / min duration / padding and Run. Video stream is re-cut to follow the audio.</span>
                {:else if selectedOperation === 'remove-audio'}
                  Strip all audio tracks from a container while stream-copying the video. Uses <strong class="text-white/80">-map 0 -map -0:a -c copy</strong>. Output has the same video codec, container, and metadata — no re-encode.
                  <br/><br/><span class="text-white/35">Click Run. Subtitles / chapters / attachments pass through unchanged.</span>
                {:else if selectedOperation === 'remove-video'}
                  Drop the video track and keep audio stream-copied. Output container is an audio-only Matroska (.mka) because it accepts any audio codec losslessly.
                  <br/><br/><span class="text-white/35">Click Run. For a specific audio container (mp3/aac/flac), use Extract instead.</span>
                {:else if selectedOperation === 'metadata-strip'}
                  Remove container-level metadata and chapters via <strong class="text-white/80">-map_metadata -1</strong>. Streams are copied untouched. Choose whether to wipe every tag, or wipe everything and re-write only a new <code>title</code>.
                  <br/><br/><span class="text-white/35">Note: this does not touch per-stream metadata inside codec bitstreams (e.g. embedded ID3 inside MP3).</span>
                {:else if selectedOperation === 'loop'}
                  Repeat the clip <strong class="text-white/80">N</strong> times end-to-end via <code>-stream_loop</code>. Streams are copied — no re-encode — so output file size is roughly N × input.
                  <br/><br/><span class="text-white/35">Allowed range: 2–50 playthroughs.</span>
                {:else if selectedOperation === 'rotate-flip'}
                  Rotate 90°/180° or mirror horizontally/vertically. Uses <code>transpose</code>, <code>hflip</code>, or <code>vflip</code> filters and re-encodes to H.264 / AAC.
                  <br/><br/><span class="text-white/35">Pick a direction and Run. 180° is two transpose=1 passes.</span>
                {:else if selectedOperation === 'speed'}
                  Change playback rate with pitch-preserved audio, or play the clip backwards. Uses <code>setpts=PTS/R</code> for video and chained <code>atempo</code> for audio (atempo's 0.5–2.0 range is handled automatically). Reverse uses <code>reverse</code> + <code>areverse</code> and is <strong class="text-white/80">memory-heavy</strong>.
                  <br/><br/><span class="text-white/35">Pick a preset or enter a custom rate (0.1–10×). Reverse runs independently; trim first for long clips.</span>
                {:else if selectedOperation === 'fade'}
                  Crop uses the timeline's trim handles. Add a fade-in from black and/or a fade-out to black at the tail — <code>fade=t=in</code> / <code>fade=t=out</code> plus matching <code>afade</code>.
                  <br/><br/><span class="text-white/35">Set the fade values; use the timeline handles for crop. Either fade defaults to 0.5s; set to 0 to skip.</span>
                {:else if selectedOperation === 'deinterlace'}
                  Convert interlaced footage to progressive. <strong class="text-white/80">yadif</strong> is fast & safe; <strong class="text-white/80">yadif 2×</strong> doubles the framerate using both fields; <strong class="text-white/80">bwdif</strong> is higher-quality at ~2× the CPU cost.
                  <br/><br/><span class="text-white/35">Use yadif for 1080i broadcast; bwdif when artifacts matter.</span>
                {:else if selectedOperation === 'denoise'}
                  Temporal + spatial noise reduction via <code>hqdn3d</code>. Three presets (light/medium/strong) map to escalating luma/chroma spatial and temporal coefficients.
                  <br/><br/><span class="text-white/35">Stronger = softer image. Watch for detail loss on faces and textures.</span>
                {:else if selectedOperation === 'thumbnail'}
                  Seek to a timestamp and write a single frame as JPEG / PNG / WebP. Uses <code>-ss T -frames:v 1</code>.
                  <br/><br/><span class="text-white/35">Time accepts <code>HH:MM:SS.ms</code> or plain seconds.</span>
                {:else if selectedOperation === 'contact-sheet'}
                  Tile N frames into a single PNG grid. Uses <code>select</code> + <code>tile</code>. Handy for quickly previewing a long clip's content.
                  <br/><br/><span class="text-white/35">Default grid: 4×6 (24 frames). Increase frames to oversample.</span>
                {:else if selectedOperation === 'frame-export'}
                  Emit an image sequence to a sibling <code>&lt;name&gt;_frames/</code> folder. Choose a fixed frames-per-second rate or a fixed time interval between frames.
                  <br/><br/><span class="text-white/35">Output pattern: <code>frame_000001.jpg</code> etc. Be mindful — this can produce thousands of files.</span>
                {:else if selectedOperation === 'watermark'}
                  Overlay a PNG watermark at a chosen corner with opacity and size (% of video width). Audio is stream-copied; video re-encodes to H.264.
                  <br/><br/><span class="text-white/35">Transparent PNGs work best. Use opacity to further tame a logo.</span>
                {:else if selectedOperation === 'channel-tools'}
                  Channel manipulations via the <code>pan</code> filter. Downmix stereo to mono, swap L/R, mute a side, or fake stereo from a mono source.
                  <br/><br/><span class="text-white/35">For true upmix/downmix with proper LFE handling, use a dedicated audio editor.</span>
                {:else if selectedOperation === 'pad-silence'}
                  Prepend or append silence. Uses <code>adelay</code> for head padding and <code>apad=pad_dur</code> for tail padding.
                  <br/><br/><span class="text-white/35">Each side: 0–60 seconds. Output is longer than the input by head + tail.</span>
                {:else if selectedOperation === 'chroma-ffmpeg'}
                  Remove a solid background colour using an FFmpeg built-in filter. <strong class="text-white/80">chromakey</strong> (YUV) is the default for green/blue screens; <strong class="text-white/80">colorkey</strong> (RGB) gives hard cuts for flat mattes; <strong class="text-white/80">hsvkey</strong> handles uneven screen lighting. Optional <code>despill</code> removes coloured light bouncing onto the subject. Output is an alpha-capable container — ProRes 4444, VP9+alpha, PNG sequence, QtRLE, or FFV1.
                  <br/><br/><span class="text-white/35">Pick a colour, tune similarity/blend, scrub to a frame and Preview. Trim handles limit the keyed segment.</span>
                {:else if selectedOperation === 'loudness'}
                  Measure <strong class="text-white/80">EBU R128</strong> loudness: integrated LUFS (I), loudness range (LRA), and true-peak (dBTP). Read-only analysis — no file is written. True-peak uses 4× oversampling for accuracy and is slower.
                  <br/><br/><span class="text-white/35">Pick a target preset (broadcast / streaming / Spotify) and Analyze. Results appear below.</span>
                {:else if selectedOperation === 'audio-norm'}
                  Normalize audio loudness. <strong class="text-white/80">EBU R128</strong> (two-pass loudnorm, recommended) measures first then applies a matched gain curve; <strong class="text-white/80">Peak</strong> scales the max sample to a ceiling; <strong class="text-white/80">ReplayGain</strong> writes tags without touching samples.
                  <br/><br/><span class="text-white/35">Choose mode, set targets, Run. Video streams are stream-copied.</span>
                {:else if selectedOperation === 'cut-detect'}
                  Find shot/scene changes. <strong class="text-white/80">scdet</strong> (FFmpeg ≥4.4) scores each frame transition; <strong class="text-white/80">scene</strong> uses the classic select filter. Higher threshold = fewer false positives. Downscale first for a 10× speedup.
                  <br/><br/><span class="text-white/35">Results list timestamps — click any entry to seek the timeline.</span>
                {:else if selectedOperation === 'black-detect'}
                  Detect black intervals (fades, slates, leader). Emits start/end/duration for each run longer than the min-duration threshold. Pairs naturally with cut detection for fade-to-black transitions.
                  <br/><br/><span class="text-white/35">Tune min duration and pixel/picture thresholds, then Analyze.</span>
                {:else if selectedOperation === 'vmaf'}
                  Netflix <strong class="text-white/80">VMAF</strong> perceptual quality score comparing a distorted encode against a reference. Outputs a mean score 0–100. Reference and distorted must match in resolution and frame rate — the tool auto-scales both inputs to the model's native size.
                  <br/><br/><span class="text-white/35">Drop the reference and distorted files, pick a model, Run. Subsample &gt;1 skips frames for speed.</span>
                {:else if selectedOperation === 'framemd5'}
                  Emit an MD5 hash per decoded frame for bit-exactness verification. Hashes are over decoded pixels (or PCM samples) so they're codec-agnostic. Diff mode compares two files line-by-line to locate the first frame of divergence.
                  <br/><br/><span class="text-white/35">Pick streams (video / audio / both). Add a second file to run a diff.</span>
                {/if}
              </p>
            </div>

            <div class="flex flex-col gap-3 w-[560px] shrink-0">
              <p class="text-[10px] uppercase tracking-wider text-white/30 font-semibold">Controls</p>
              <div class="flex flex-wrap gap-2">
                {#if selectedOperation === 'cut-noenc'}
                  <!-- Cut/Extract segmented toggle — matches other ops' style -->
                  <div class="inline-flex items-center rounded-md overflow-hidden border border-[var(--border)]">
                    <button
                      onclick={() => cutMode = 'cut'}
                      class="px-3 py-1.5 text-[12px] font-semibold transition-colors
                             {cutMode === 'cut' ? 'bg-[var(--accent)] text-white' : 'text-white/60 hover:bg-white/5'}"
                    >Cut</button>
                    <div class="w-px h-6 bg-[var(--border)]"></div>
                    <button
                      onclick={() => cutMode = 'extract'}
                      class="px-3 py-1.5 text-[12px] font-semibold transition-colors
                             {cutMode === 'extract' ? 'bg-[var(--accent)] text-white' : 'text-white/60 hover:bg-white/5'}"
                    >Extract</button>
                  </div>
                  <button
                    onclick={() => cutMode === 'cut' ? runCut() : runExtract()}
                    disabled={!selectedItem || selectedItem.status === 'converting'}
                    class="px-3 py-1.5 rounded text-[12px] font-semibold bg-[var(--accent)] text-white hover:opacity-90 transition-opacity disabled:opacity-40"
                  >Run {cutMode === 'cut' ? 'Cut' : 'Extract'}</button>
                {:else if selectedOperation === 'replace-audio'}
                  <!-- Row 1: file + track + stretch toggle -->
                  <div class="flex flex-wrap items-center gap-2 w-full">
                    <button
                      onclick={() => replaceAudioPath
                        ? (replaceAudioPath = null)
                        : pickAuxFile((p) => replaceAudioPath = p, 'audio/*,video/*')}
                      class="px-3 py-1.5 rounded text-[12px] font-medium border border-[var(--border)] text-[var(--text-secondary)] hover:text-[var(--text-primary)] hover:border-[var(--accent)] transition-colors"
                    >{replaceAudioPath ? 'Clear replacement' : 'Pick audio file…'}</button>
                    <button class="px-3 py-1.5 rounded text-[12px] font-medium border border-[var(--border)] text-[var(--text-secondary)] hover:text-[var(--text-primary)] hover:border-[var(--accent)] transition-colors">Keep original tracks</button>
                    <!-- One-shot auto-sync: xcorr alignment + pitch-preserved stretch + SR/codec match -->
                    <button
                      onclick={() => replaceAudioAutoSync = !replaceAudioAutoSync}
                      title="Cross-correlate to find offset, pitch-preserved stretch to match length, and reuse the video's existing audio sample-rate & codec."
                      class="px-3 py-1.5 rounded text-[12px] font-semibold border transition-colors
                             {replaceAudioAutoSync
                               ? 'bg-[var(--accent)] border-[var(--accent)] text-white'
                               : 'border-[var(--accent)] text-[var(--accent)] hover:bg-[color-mix(in_srgb,var(--accent)_12%,transparent)]'}"
                    >✦ Auto-sync</button>
                    <!-- Stretch segmented toggle: Off / Fit to length -->
                    <div class="inline-flex items-center rounded-md overflow-hidden border border-[var(--border)]">
                      <span class="px-2 text-[10px] uppercase tracking-wider text-white/40 font-semibold">Stretch</span>
                      <div class="w-px h-6 bg-[var(--border)]"></div>
                      <button
                        onclick={() => replaceAudioFitLength = false}
                        class="px-3 py-1.5 text-[12px] font-semibold transition-colors
                               {!replaceAudioFitLength ? 'bg-[var(--accent)] text-white' : 'text-white/60 hover:bg-white/5'}"
                      >Off</button>
                      <div class="w-px h-6 bg-[var(--border)]"></div>
                      <button
                        onclick={() => replaceAudioFitLength = true}
                        class="px-3 py-1.5 text-[12px] font-semibold transition-colors
                               {replaceAudioFitLength ? 'bg-[var(--accent)] text-white' : 'text-white/60 hover:bg-white/5'}"
                      >Fit to length</button>
                    </div>
                  </div>
                  <!-- Row 2: Run Replace · long offset slider · numeric readout -->
                  <div class="flex items-center gap-3 w-full">
                    <button
                      onclick={runReplaceAudio}
                      disabled={!selectedItem || selectedItem.mediaType !== 'video' || !replaceAudioPath || selectedItem.status === 'converting'}
                      class="shrink-0 px-3 py-1.5 rounded text-[12px] font-semibold bg-[var(--accent)] text-white hover:opacity-90 transition-opacity disabled:opacity-40"
                    >Run Replace</button>
                    <div class="flex items-center gap-2 flex-1 min-w-0">
                      <span class="text-[10px] uppercase tracking-wider text-white/40 font-semibold shrink-0">Offset</span>
                      <input
                        type="range"
                        min="-2000"
                        max="2000"
                        step="10"
                        bind:value={replaceAudioOffsetMs}
                        ondblclick={() => replaceAudioOffsetMs = 0}
                        class="flex-1 min-w-0 accent-[var(--accent)]"
                        title="Double-click to reset to 0"
                      />
                      <input
                        type="number"
                        step="10"
                        bind:value={replaceAudioOffsetMs}
                        class="w-16 shrink-0 bg-black/30 border border-[var(--border)] rounded px-1.5 py-0.5 text-[12px] text-white outline-none text-right font-mono tabular-nums focus:border-[var(--accent)]"
                      />
                      <span class="text-[10px] text-white/30 shrink-0">ms</span>
                    </div>
                  </div>
                {:else if selectedOperation === 'rewrap'}
                  {#each ['mp4', 'mkv', 'mov', 'webm'] as fmt}
                    <button
                      onclick={() => rewrapFormat = fmt}
                      class="px-3 py-1.5 rounded text-[12px] font-semibold border transition-colors
                             {rewrapFormat === fmt
                               ? 'bg-[var(--accent)] border-[var(--accent)] text-white'
                               : 'border-[var(--border)] text-[var(--text-secondary)] hover:text-[var(--text-primary)] hover:border-[var(--accent)]'}"
                    >{fmt.toUpperCase()}</button>
                  {/each}
                  <button
                    onclick={runRewrap}
                    disabled={!selectedItem || selectedItem.status === 'converting'}
                    class="px-3 py-1.5 rounded text-[12px] font-semibold bg-[var(--accent)] text-white hover:opacity-90 transition-opacity disabled:opacity-40"
                  >Run Rewrap</button>
                {:else if selectedOperation === 'conform'}
                  <!-- Row 1: targets -->
                  <div class="flex flex-wrap items-center gap-2 w-full">
                    <div class="flex items-center gap-1.5 rounded border border-[var(--border)] px-2 py-1">
                      <label class="text-[10px] uppercase tracking-wider text-white/40 font-semibold">FPS</label>
                      <select bind:value={conformFps}
                              class="bg-transparent text-[12px] text-white outline-none font-mono tabular-nums">
                        <option value="source">source</option>
                        <option value="23.976">23.976</option>
                        <option value="24">24</option>
                        <option value="25">25</option>
                        <option value="29.97">29.97</option>
                        <option value="30">30</option>
                        <option value="50">50</option>
                        <option value="59.94">59.94</option>
                        <option value="60">60</option>
                      </select>
                    </div>
                    <div class="flex items-center gap-1.5 rounded border border-[var(--border)] px-2 py-1">
                      <label class="text-[10px] uppercase tracking-wider text-white/40 font-semibold">Res</label>
                      <select bind:value={conformResolution}
                              class="bg-transparent text-[12px] text-white outline-none font-mono tabular-nums">
                        <option value="source">source</option>
                        <option value="3840x2160">3840×2160 (UHD)</option>
                        <option value="1920x1080">1920×1080 (HD)</option>
                        <option value="1280x720">1280×720</option>
                        <option value="854x480">854×480</option>
                      </select>
                    </div>
                    <div class="flex items-center gap-1.5 rounded border border-[var(--border)] px-2 py-1">
                      <label class="text-[10px] uppercase tracking-wider text-white/40 font-semibold">Pix</label>
                      <select bind:value={conformPixFmt}
                              class="bg-transparent text-[12px] text-white outline-none font-mono tabular-nums">
                        <option value="source">source</option>
                        <option value="yuv420p">yuv420p (8-bit)</option>
                        <option value="yuv420p10le">yuv420p10le (10-bit)</option>
                        <option value="yuv422p">yuv422p (8-bit)</option>
                        <option value="yuv422p10le">yuv422p10le (10-bit)</option>
                        <option value="yuv444p">yuv444p (8-bit)</option>
                      </select>
                    </div>
                    <button class="px-3 py-1.5 rounded text-[12px] font-medium border border-[var(--border)] text-[var(--text-secondary)] hover:text-[var(--text-primary)] hover:border-[var(--accent)] transition-colors"
                            title="Pick a reference video and copy its fps/resolution/pix_fmt into the fields above.">Match reference…</button>
                  </div>
                  <!-- Row 2: algorithms -->
                  <div class="flex flex-wrap items-center gap-2 w-full">
                    <!-- FPS conversion algorithm — segmented -->
                    <div class="inline-flex items-center rounded-md overflow-hidden border border-[var(--border)]">
                      <span class="px-2 text-[10px] uppercase tracking-wider text-white/40 font-semibold">FPS algo</span>
                      <div class="w-px h-6 bg-[var(--border)]"></div>
                      <button onclick={() => conformFpsAlgo = 'drop'}
                              title="Drop / duplicate frames. Fast, deterministic. Judder on non-integer ratios."
                              class="px-2.5 py-1.5 text-[11px] font-semibold transition-colors
                                     {conformFpsAlgo === 'drop' ? 'bg-[var(--accent)] text-white' : 'text-white/60 hover:bg-white/5'}">Drop/dup</button>
                      <div class="w-px h-6 bg-[var(--border)]"></div>
                      <button onclick={() => conformFpsAlgo = 'blend'}
                              title="Blend adjacent frames. Smoother than drop/dup, but ghosting on motion."
                              class="px-2.5 py-1.5 text-[11px] font-semibold transition-colors
                                     {conformFpsAlgo === 'blend' ? 'bg-[var(--accent)] text-white' : 'text-white/60 hover:bg-white/5'}">Blend</button>
                      <div class="w-px h-6 bg-[var(--border)]"></div>
                      <button onclick={() => conformFpsAlgo = 'mci'}
                              title="Motion-compensated interpolation (minterpolate). Highest quality, 5–30× slower. May warp on complex motion / occlusions."
                              class="px-2.5 py-1.5 text-[11px] font-semibold transition-colors
                                     {conformFpsAlgo === 'mci' ? 'bg-[var(--accent)] text-white' : 'text-white/60 hover:bg-white/5'}">
                        Optical flow
                        {#if conformFpsAlgo === 'mci'}<span class="ml-1 text-[9px] opacity-75">slow</span>{/if}
                      </button>
                    </div>
                    <!-- Scale filter -->
                    <div class="flex items-center gap-1.5 rounded border border-[var(--border)] px-2 py-1">
                      <label class="text-[10px] uppercase tracking-wider text-white/40 font-semibold">Scale</label>
                      <select bind:value={conformScaleAlgo}
                              title="Bilinear: fast, soft. Bicubic: balanced. Lanczos: sharp downscale, ringing risk upscale. Spline: smoother upscale."
                              class="bg-transparent text-[12px] text-white outline-none">
                        <option value="bilinear">Bilinear</option>
                        <option value="bicubic">Bicubic</option>
                        <option value="lanczos">Lanczos</option>
                        <option value="spline">Spline</option>
                      </select>
                    </div>
                    <!-- Dither -->
                    <label class="flex items-center gap-1.5 rounded border border-[var(--border)] px-2 py-1 cursor-pointer"
                           title="Apply error-diffusion dither when converting 10-bit → 8-bit. Prevents banding in gradients. Auto-applied only if source and target depths differ.">
                      <input type="checkbox" bind:checked={conformDither}
                             class="accent-[var(--accent)]"/>
                      <span class="text-[11px] text-white/70 font-medium">10→8 dither</span>
                    </label>
                  </div>
                  <!-- Row 3: run -->
                  <div class="w-full">
                    <button
                      onclick={runConform}
                      disabled={!selectedItem || selectedItem.mediaType !== 'video' || selectedItem.status === 'converting'}
                      class="px-3 py-1.5 rounded text-[12px] font-semibold bg-[var(--accent)] text-white hover:opacity-90 transition-opacity disabled:opacity-40 disabled:cursor-not-allowed"
                    >Run Conform</button>
                  </div>
                {:else if selectedOperation === 'silence-remove'}
                  <div class="flex flex-wrap items-center gap-2 w-full">
                    <div class="flex items-center gap-1.5 rounded border border-[var(--border)] px-2 py-1">
                      <label class="text-[10px] uppercase tracking-wider text-white/40 font-semibold">Threshold (dB)</label>
                      <input type="number" step="1" bind:value={silenceThresholdDb}
                             class="w-14 bg-transparent text-[12px] text-white outline-none text-right font-mono tabular-nums"/>
                    </div>
                    <div class="flex items-center gap-1.5 rounded border border-[var(--border)] px-2 py-1">
                      <label class="text-[10px] uppercase tracking-wider text-white/40 font-semibold">Min silence (s)</label>
                      <input type="number" step="0.05" bind:value={silenceMinDurS}
                             class="w-14 bg-transparent text-[12px] text-white outline-none text-right font-mono tabular-nums"/>
                    </div>
                    <div class="flex items-center gap-1.5 rounded border border-[var(--border)] px-2 py-1">
                      <label class="text-[10px] uppercase tracking-wider text-white/40 font-semibold">Pad (ms)</label>
                      <input type="number" step="10" bind:value={silencePadMs}
                             class="w-14 bg-transparent text-[12px] text-white outline-none text-right font-mono tabular-nums"/>
                    </div>
                  </div>
                  <div class="w-full">
                    <button
                      onclick={runSilenceRemove}
                      disabled={!selectedItem || selectedItem.status === 'converting'}
                      class="px-3 py-1.5 rounded text-[12px] font-semibold bg-[var(--accent)] text-white hover:opacity-90 transition-opacity disabled:opacity-40"
                    >Run Silence Remover</button>
                  </div>
                {:else if selectedOperation === 'remove-audio'}
                  <div class="w-full">
                    <button
                      onclick={runRemoveAudio}
                      disabled={!selectedItem || selectedItem.status === 'converting'}
                      class="px-3 py-1.5 rounded text-[12px] font-semibold bg-[var(--accent)] text-white hover:opacity-90 transition-opacity disabled:opacity-40"
                    >Run Remove Audio</button>
                  </div>
                {:else if selectedOperation === 'remove-video'}
                  <div class="w-full">
                    <button
                      onclick={runRemoveVideo}
                      disabled={!selectedItem || selectedItem.status === 'converting'}
                      class="px-3 py-1.5 rounded text-[12px] font-semibold bg-[var(--accent)] text-white hover:opacity-90 transition-opacity disabled:opacity-40"
                    >Run Remove Video</button>
                  </div>
                {:else if selectedOperation === 'metadata-strip'}
                  <div class="flex flex-wrap items-center gap-2 w-full">
                    <div class="inline-flex items-center rounded-md overflow-hidden border border-[var(--border)]">
                      <button onclick={() => metadataStripMode = 'all'}
                        class="px-3 py-1.5 text-[12px] font-semibold transition-colors
                               {metadataStripMode === 'all' ? 'bg-[var(--accent)] text-white' : 'text-white/60 hover:bg-white/5'}"
                      >Strip all</button>
                      <div class="w-px h-6 bg-[var(--border)]"></div>
                      <button onclick={() => metadataStripMode = 'title'}
                        class="px-3 py-1.5 text-[12px] font-semibold transition-colors
                               {metadataStripMode === 'title' ? 'bg-[var(--accent)] text-white' : 'text-white/60 hover:bg-white/5'}"
                      >Keep title only</button>
                    </div>
                    {#if metadataStripMode === 'title'}
                      <div class="flex items-center gap-1.5 rounded border border-[var(--border)] px-2 py-1 flex-1 min-w-[140px]">
                        <label class="text-[10px] uppercase tracking-wider text-white/40 font-semibold">Title</label>
                        <input type="text" bind:value={metadataStripTitle}
                               placeholder="New title"
                               class="flex-1 bg-transparent text-[12px] text-white outline-none"/>
                      </div>
                    {/if}
                  </div>
                  <div class="w-full">
                    <button
                      onclick={runMetadataStrip}
                      disabled={!selectedItem || selectedItem.status === 'converting'}
                      class="px-3 py-1.5 rounded text-[12px] font-semibold bg-[var(--accent)] text-white hover:opacity-90 transition-opacity disabled:opacity-40"
                    >Run Strip Metadata</button>
                  </div>
                {:else if selectedOperation === 'loop'}
                  <div class="flex flex-wrap items-center gap-2 w-full">
                    <div class="flex items-center gap-1.5 rounded border border-[var(--border)] px-2 py-1">
                      <label class="text-[10px] uppercase tracking-wider text-white/40 font-semibold">Count</label>
                      <input type="number" min="2" max="50" step="1" bind:value={loopCount}
                             class="w-14 bg-transparent text-[12px] text-white outline-none text-right font-mono tabular-nums"/>
                    </div>
                  </div>
                  <div class="w-full">
                    <button
                      onclick={runLoop}
                      disabled={!selectedItem || selectedItem.status === 'converting'}
                      class="px-3 py-1.5 rounded text-[12px] font-semibold bg-[var(--accent)] text-white hover:opacity-90 transition-opacity disabled:opacity-40"
                    >Run Loop</button>
                  </div>
                {:else if selectedOperation === 'rotate-flip'}
                  <div class="flex flex-wrap items-center gap-2 w-full">
                    <div class="inline-flex items-center rounded-md overflow-hidden border border-[var(--border)]">
                      {#each [['cw90','90° CW'],['ccw90','90° CCW'],['180','180°'],['hflip','Flip H'],['vflip','Flip V']] as [id, label], i}
                        {#if i > 0}<div class="w-px h-6 bg-[var(--border)]"></div>{/if}
                        <button onclick={() => rotateFlipMode = id}
                          class="px-3 py-1.5 text-[12px] font-semibold transition-colors
                                 {rotateFlipMode === id ? 'bg-[var(--accent)] text-white' : 'text-white/60 hover:bg-white/5'}"
                        >{label}</button>
                      {/each}
                    </div>
                  </div>
                  <div class="w-full">
                    <button onclick={runRotateFlip}
                      disabled={!selectedItem || selectedItem.mediaType !== 'video' || selectedItem.status === 'converting'}
                      class="px-3 py-1.5 rounded text-[12px] font-semibold bg-[var(--accent)] text-white hover:opacity-90 transition-opacity disabled:opacity-40"
                    >Run Rotate/Flip</button>
                  </div>
                {:else if selectedOperation === 'speed'}
                  <div class="flex flex-wrap items-center gap-2 w-full">
                    <div class="inline-flex items-center rounded-md overflow-hidden border border-[var(--border)]">
                      {#each ['0.5','0.75','1','1.25','1.5','2','custom'] as p, i}
                        {#if i > 0}<div class="w-px h-6 bg-[var(--border)]"></div>{/if}
                        <button onclick={() => speedPreset = p}
                          class="px-2.5 py-1.5 text-[11px] font-semibold transition-colors
                                 {speedPreset === p ? 'bg-[var(--accent)] text-white' : 'text-white/60 hover:bg-white/5'}"
                        >{p === 'custom' ? 'Custom' : p + 'x'}</button>
                      {/each}
                    </div>
                    {#if speedPreset === 'custom'}
                      <div class="flex items-center gap-1.5 rounded border border-[var(--border)] px-2 py-1">
                        <label class="text-[10px] uppercase tracking-wider text-white/40 font-semibold">Rate</label>
                        <input type="number" min="0.1" max="10" step="0.05" bind:value={speedCustom}
                               class="w-16 bg-transparent text-[12px] text-white outline-none text-right font-mono tabular-nums"/>
                      </div>
                    {/if}
                  </div>
                  <div class="w-full flex items-center gap-2">
                    <button onclick={runSpeed}
                      disabled={!selectedItem || selectedItem.status === 'converting'}
                      class="px-3 py-1.5 rounded text-[12px] font-semibold bg-[var(--accent)] text-white hover:opacity-90 transition-opacity disabled:opacity-40"
                    >Run Speed</button>
                    <button onclick={runReverse}
                      disabled={!selectedItem || selectedItem.status === 'converting'}
                      class="px-3 py-1.5 rounded text-[12px] font-semibold border border-[var(--accent)] text-[var(--accent)] hover:bg-[var(--accent)] hover:text-white transition-colors disabled:opacity-40"
                    >Run Reverse</button>
                  </div>
                {:else if selectedOperation === 'fade'}
                  <div class="flex flex-wrap items-center gap-2 w-full">
                    <div class="flex items-center gap-1.5 rounded border border-[var(--border)] px-2 py-1">
                      <label class="text-[10px] uppercase tracking-wider text-white/40 font-semibold">In (s)</label>
                      <input type="number" min="0" max="10" step="0.1" bind:value={fadeInS}
                             class="w-14 bg-transparent text-[12px] text-white outline-none text-right font-mono tabular-nums"/>
                    </div>
                    <div class="flex items-center gap-1.5 rounded border border-[var(--border)] px-2 py-1">
                      <label class="text-[10px] uppercase tracking-wider text-white/40 font-semibold">Out (s)</label>
                      <input type="number" min="0" max="10" step="0.1" bind:value={fadeOutS}
                             class="w-14 bg-transparent text-[12px] text-white outline-none text-right font-mono tabular-nums"/>
                    </div>
                  </div>
                  <div class="w-full">
                    <button onclick={runFade}
                      disabled={!selectedItem || selectedItem.status === 'converting'}
                      class="px-3 py-1.5 rounded text-[12px] font-semibold bg-[var(--accent)] text-white hover:opacity-90 transition-opacity disabled:opacity-40"
                    >Run Fade</button>
                  </div>
                {:else if selectedOperation === 'deinterlace'}
                  <div class="flex flex-wrap items-center gap-2 w-full">
                    <div class="inline-flex items-center rounded-md overflow-hidden border border-[var(--border)]">
                      {#each [['yadif','yadif'],['yadif_double','yadif 2×'],['bwdif','bwdif']] as [id, label], i}
                        {#if i > 0}<div class="w-px h-6 bg-[var(--border)]"></div>{/if}
                        <button onclick={() => deinterlaceMode = id}
                          class="px-3 py-1.5 text-[12px] font-semibold transition-colors
                                 {deinterlaceMode === id ? 'bg-[var(--accent)] text-white' : 'text-white/60 hover:bg-white/5'}"
                        >{label}</button>
                      {/each}
                    </div>
                  </div>
                  <div class="w-full">
                    <button onclick={runDeinterlace}
                      disabled={!selectedItem || selectedItem.mediaType !== 'video' || selectedItem.status === 'converting'}
                      class="px-3 py-1.5 rounded text-[12px] font-semibold bg-[var(--accent)] text-white hover:opacity-90 transition-opacity disabled:opacity-40"
                    >Run Deinterlace</button>
                  </div>
                {:else if selectedOperation === 'denoise'}
                  <div class="flex flex-wrap items-center gap-2 w-full">
                    <div class="inline-flex items-center rounded-md overflow-hidden border border-[var(--border)]">
                      {#each ['light','medium','strong'] as p, i}
                        {#if i > 0}<div class="w-px h-6 bg-[var(--border)]"></div>{/if}
                        <button onclick={() => denoisePreset = p}
                          class="px-3 py-1.5 text-[12px] font-semibold capitalize transition-colors
                                 {denoisePreset === p ? 'bg-[var(--accent)] text-white' : 'text-white/60 hover:bg-white/5'}"
                        >{p}</button>
                      {/each}
                    </div>
                  </div>
                  <div class="w-full">
                    <button onclick={runDenoise}
                      disabled={!selectedItem || selectedItem.mediaType !== 'video' || selectedItem.status === 'converting'}
                      class="px-3 py-1.5 rounded text-[12px] font-semibold bg-[var(--accent)] text-white hover:opacity-90 transition-opacity disabled:opacity-40"
                    >Run Denoise</button>
                  </div>
                {:else if selectedOperation === 'thumbnail'}
                  <div class="flex flex-wrap items-center gap-2 w-full">
                    <div class="flex items-center gap-1.5 rounded border border-[var(--border)] px-2 py-1">
                      <label class="text-[10px] uppercase tracking-wider text-white/40 font-semibold">Time</label>
                      <input type="text" bind:value={thumbnailTime}
                             placeholder="00:00:01"
                             class="w-24 bg-transparent text-[12px] text-white outline-none text-right font-mono tabular-nums"/>
                    </div>
                    <div class="inline-flex items-center rounded-md overflow-hidden border border-[var(--border)]">
                      {#each ['jpeg','png','webp'] as p, i}
                        {#if i > 0}<div class="w-px h-6 bg-[var(--border)]"></div>{/if}
                        <button onclick={() => thumbnailFormat = p}
                          class="px-3 py-1.5 text-[12px] font-semibold uppercase transition-colors
                                 {thumbnailFormat === p ? 'bg-[var(--accent)] text-white' : 'text-white/60 hover:bg-white/5'}"
                        >{p}</button>
                      {/each}
                    </div>
                  </div>
                  <div class="w-full">
                    <button onclick={runThumbnail}
                      disabled={!selectedItem || selectedItem.status === 'converting'}
                      class="px-3 py-1.5 rounded text-[12px] font-semibold bg-[var(--accent)] text-white hover:opacity-90 transition-opacity disabled:opacity-40"
                    >Run Thumbnail</button>
                  </div>
                {:else if selectedOperation === 'contact-sheet'}
                  <div class="flex flex-wrap items-center gap-2 w-full">
                    <div class="flex items-center gap-1.5 rounded border border-[var(--border)] px-2 py-1">
                      <label class="text-[10px] uppercase tracking-wider text-white/40 font-semibold">Cols</label>
                      <input type="number" min="1" max="20" step="1" bind:value={contactGridCols}
                             class="w-12 bg-transparent text-[12px] text-white outline-none text-right font-mono tabular-nums"/>
                    </div>
                    <div class="flex items-center gap-1.5 rounded border border-[var(--border)] px-2 py-1">
                      <label class="text-[10px] uppercase tracking-wider text-white/40 font-semibold">Rows</label>
                      <input type="number" min="1" max="20" step="1" bind:value={contactGridRows}
                             class="w-12 bg-transparent text-[12px] text-white outline-none text-right font-mono tabular-nums"/>
                    </div>
                    <div class="flex items-center gap-1.5 rounded border border-[var(--border)] px-2 py-1">
                      <label class="text-[10px] uppercase tracking-wider text-white/40 font-semibold">Frames</label>
                      <input type="number" min="1" max="500" step="1" bind:value={contactFrames}
                             class="w-16 bg-transparent text-[12px] text-white outline-none text-right font-mono tabular-nums"/>
                    </div>
                  </div>
                  <div class="w-full">
                    <button onclick={runContactSheet}
                      disabled={!selectedItem || selectedItem.mediaType !== 'video' || selectedItem.status === 'converting'}
                      class="px-3 py-1.5 rounded text-[12px] font-semibold bg-[var(--accent)] text-white hover:opacity-90 transition-opacity disabled:opacity-40"
                    >Run Contact Sheet</button>
                  </div>
                {:else if selectedOperation === 'frame-export'}
                  <div class="flex flex-wrap items-center gap-2 w-full">
                    <div class="inline-flex items-center rounded-md overflow-hidden border border-[var(--border)]">
                      {#each [['fps','FPS'],['interval','Interval (s)']] as [id, label], i}
                        {#if i > 0}<div class="w-px h-6 bg-[var(--border)]"></div>{/if}
                        <button onclick={() => frameExportMode = id}
                          class="px-3 py-1.5 text-[12px] font-semibold transition-colors
                                 {frameExportMode === id ? 'bg-[var(--accent)] text-white' : 'text-white/60 hover:bg-white/5'}"
                        >{label}</button>
                      {/each}
                    </div>
                    {#if frameExportMode === 'fps'}
                      <div class="flex items-center gap-1.5 rounded border border-[var(--border)] px-2 py-1">
                        <label class="text-[10px] uppercase tracking-wider text-white/40 font-semibold">FPS</label>
                        <input type="number" min="0.01" step="0.1" bind:value={frameExportFps}
                               class="w-16 bg-transparent text-[12px] text-white outline-none text-right font-mono tabular-nums"/>
                      </div>
                    {:else}
                      <div class="flex items-center gap-1.5 rounded border border-[var(--border)] px-2 py-1">
                        <label class="text-[10px] uppercase tracking-wider text-white/40 font-semibold">Interval</label>
                        <input type="number" min="0.01" step="0.5" bind:value={frameExportInterval}
                               class="w-16 bg-transparent text-[12px] text-white outline-none text-right font-mono tabular-nums"/>
                        <span class="text-[10px] text-white/30">s</span>
                      </div>
                    {/if}
                    <div class="inline-flex items-center rounded-md overflow-hidden border border-[var(--border)]">
                      {#each ['jpeg','png','webp'] as p, i}
                        {#if i > 0}<div class="w-px h-6 bg-[var(--border)]"></div>{/if}
                        <button onclick={() => frameExportFormat = p}
                          class="px-3 py-1.5 text-[12px] font-semibold uppercase transition-colors
                                 {frameExportFormat === p ? 'bg-[var(--accent)] text-white' : 'text-white/60 hover:bg-white/5'}"
                        >{p}</button>
                      {/each}
                    </div>
                  </div>
                  <div class="w-full">
                    <button onclick={runFrameExport}
                      disabled={!selectedItem || selectedItem.mediaType !== 'video' || selectedItem.status === 'converting'}
                      class="px-3 py-1.5 rounded text-[12px] font-semibold bg-[var(--accent)] text-white hover:opacity-90 transition-opacity disabled:opacity-40"
                    >Run Frame Export</button>
                  </div>
                {:else if selectedOperation === 'watermark'}
                  <div class="flex flex-wrap items-center gap-2 w-full">
                    <button
                      onclick={() => watermarkPath
                        ? (watermarkPath = null)
                        : pickAuxFile((p) => watermarkPath = p, 'image/png,image/*')}
                      class="px-3 py-1.5 rounded text-[12px] font-medium border border-[var(--border)] text-[var(--text-secondary)] hover:text-[var(--text-primary)] hover:border-[var(--accent)] transition-colors"
                    >{watermarkPath ? 'Clear watermark' : 'Pick PNG…'}</button>
                    <div class="inline-flex items-center rounded-md overflow-hidden border border-[var(--border)]">
                      <span class="px-2 text-[10px] uppercase tracking-wider text-white/40 font-semibold">Corner</span>
                      {#each [['tl','TL'],['tr','TR'],['bl','BL'],['br','BR'],['center','Ctr']] as [id, label]}
                        <div class="w-px h-6 bg-[var(--border)]"></div>
                        <button onclick={() => watermarkCorner = id}
                          class="px-2.5 py-1.5 text-[11px] font-semibold transition-colors
                                 {watermarkCorner === id ? 'bg-[var(--accent)] text-white' : 'text-white/60 hover:bg-white/5'}"
                        >{label}</button>
                      {/each}
                    </div>
                    <div class="flex items-center gap-1.5 rounded border border-[var(--border)] px-2 py-1">
                      <label class="text-[10px] uppercase tracking-wider text-white/40 font-semibold">Opacity</label>
                      <input type="range" min="0" max="1" step="0.05" bind:value={watermarkOpacity}
                             class="w-24 accent-[var(--accent)]"/>
                      <span class="text-[11px] text-white/60 font-mono tabular-nums w-10 text-right">{Number(watermarkOpacity).toFixed(2)}</span>
                    </div>
                    <div class="flex items-center gap-1.5 rounded border border-[var(--border)] px-2 py-1">
                      <label class="text-[10px] uppercase tracking-wider text-white/40 font-semibold">Size</label>
                      <input type="number" min="1" max="100" step="1" bind:value={watermarkScale}
                             class="w-14 bg-transparent text-[12px] text-white outline-none text-right font-mono tabular-nums"/>
                      <span class="text-[10px] text-white/30">%</span>
                    </div>
                  </div>
                  <div class="w-full">
                    <button onclick={runWatermark}
                      disabled={!selectedItem || selectedItem.mediaType !== 'video' || !watermarkPath || selectedItem.status === 'converting'}
                      class="px-3 py-1.5 rounded text-[12px] font-semibold bg-[var(--accent)] text-white hover:opacity-90 transition-opacity disabled:opacity-40"
                    >Run Watermark</button>
                  </div>
                {:else if selectedOperation === 'channel-tools'}
                  <div class="flex flex-wrap items-center gap-2 w-full">
                    <div class="inline-flex items-center rounded-md overflow-hidden border border-[var(--border)] flex-wrap">
                      {#each [['stereo_to_mono','Stereo→Mono'],['swap','Swap L↔R'],['mute_l','Mute L'],['mute_r','Mute R'],['mono_to_stereo','Mono→Stereo']] as [id, label], i}
                        {#if i > 0}<div class="w-px h-6 bg-[var(--border)]"></div>{/if}
                        <button onclick={() => channelToolsMode = id}
                          class="px-3 py-1.5 text-[12px] font-semibold transition-colors
                                 {channelToolsMode === id ? 'bg-[var(--accent)] text-white' : 'text-white/60 hover:bg-white/5'}"
                        >{label}</button>
                      {/each}
                    </div>
                  </div>
                  <div class="w-full">
                    <button onclick={runChannelTools}
                      disabled={!selectedItem || selectedItem.status === 'converting'}
                      class="px-3 py-1.5 rounded text-[12px] font-semibold bg-[var(--accent)] text-white hover:opacity-90 transition-opacity disabled:opacity-40"
                    >Run Channel Tools</button>
                  </div>
                {:else if selectedOperation === 'pad-silence'}
                  <div class="flex flex-wrap items-center gap-2 w-full">
                    <div class="flex items-center gap-1.5 rounded border border-[var(--border)] px-2 py-1">
                      <label class="text-[10px] uppercase tracking-wider text-white/40 font-semibold">Head (s)</label>
                      <input type="number" min="0" max="60" step="0.1" bind:value={padSilenceHead}
                             class="w-14 bg-transparent text-[12px] text-white outline-none text-right font-mono tabular-nums"/>
                    </div>
                    <div class="flex items-center gap-1.5 rounded border border-[var(--border)] px-2 py-1">
                      <label class="text-[10px] uppercase tracking-wider text-white/40 font-semibold">Tail (s)</label>
                      <input type="number" min="0" max="60" step="0.1" bind:value={padSilenceTail}
                             class="w-14 bg-transparent text-[12px] text-white outline-none text-right font-mono tabular-nums"/>
                    </div>
                  </div>
                  <div class="w-full">
                    <button onclick={runPadSilence}
                      disabled={!selectedItem || selectedItem.status === 'converting'}
                      class="px-3 py-1.5 rounded text-[12px] font-semibold bg-[var(--accent)] text-white hover:opacity-90 transition-opacity disabled:opacity-40"
                    >Run Pad Silence</button>
                  </div>
                {:else if selectedOperation === 'chroma-ffmpeg'}
                  <!-- Row 1: algorithm segmented -->
                  <div class="flex flex-wrap items-center gap-2 w-full">
                    <div class="inline-flex items-center rounded-md overflow-hidden border border-[var(--border)]">
                      <span class="px-2 text-[10px] uppercase tracking-wider text-white/40 font-semibold">Algo</span>
                      <div class="w-px h-6 bg-[var(--border)]"></div>
                      <button onclick={() => { chromaAlgo = 'chromakey'; generateChromaPreview(); }}
                              title="YUV-space key. Best for green / blue screens with evenly-lit backdrops."
                              class="px-2.5 py-1.5 text-[11px] font-semibold transition-colors
                                     {chromaAlgo === 'chromakey' ? 'bg-[var(--accent)] text-white' : 'text-white/60 hover:bg-white/5'}">chromakey</button>
                      <div class="w-px h-6 bg-[var(--border)]"></div>
                      <button onclick={() => { chromaAlgo = 'colorkey'; generateChromaPreview(); }}
                              title="RGB-space hard cut. Best for solid flat-colour mattes (title cards, generated BG)."
                              class="px-2.5 py-1.5 text-[11px] font-semibold transition-colors
                                     {chromaAlgo === 'colorkey' ? 'bg-[var(--accent)] text-white' : 'text-white/60 hover:bg-white/5'}">colorkey</button>
                      <div class="w-px h-6 bg-[var(--border)]"></div>
                      <button onclick={() => { chromaAlgo = 'hsvkey'; generateChromaPreview(); }}
                              title="HSV-space key. Use when lighting on the screen is uneven."
                              class="px-2.5 py-1.5 text-[11px] font-semibold transition-colors
                                     {chromaAlgo === 'hsvkey' ? 'bg-[var(--accent)] text-white' : 'text-white/60 hover:bg-white/5'}">hsvkey</button>
                    </div>
                    <label class="flex items-center gap-1.5 rounded border border-[var(--border)] px-2 py-1 cursor-pointer"
                           title="Key colour. Defaults to pure green. Use the eyedropper-style native picker.">
                      <span class="text-[10px] uppercase tracking-wider text-white/40 font-semibold">Colour</span>
                      <input type="color" bind:value={chromaColor}
                             oninput={generateChromaPreview}
                             class="w-8 h-6 rounded border border-[var(--border)] bg-transparent cursor-pointer"/>
                      <span class="text-[11px] text-white/50 font-mono tabular-nums">{chromaColor}</span>
                    </label>
                  </div>
                  <!-- Row 2: similarity slider -->
                  <div class="flex items-center gap-2 w-full">
                    <span class="text-[10px] uppercase tracking-wider text-white/40 font-semibold shrink-0 w-20">Similarity</span>
                    <input type="range" min="0.01" max="0.40" step="0.01"
                           bind:value={chromaSimilarity}
                           oninput={generateChromaPreview}
                           class="flex-1 accent-[var(--accent)]"/>
                    <span class="text-[11px] text-white/70 font-mono tabular-nums w-10 text-right">{Number(chromaSimilarity).toFixed(2)}</span>
                  </div>
                  <!-- Row 3: blend slider -->
                  <div class="flex items-center gap-2 w-full">
                    <span class="text-[10px] uppercase tracking-wider text-white/40 font-semibold shrink-0 w-20">Blend</span>
                    <input type="range" min="0.00" max="0.50" step="0.01"
                           bind:value={chromaBlend}
                           oninput={generateChromaPreview}
                           class="flex-1 accent-[var(--accent)]"/>
                    <span class="text-[11px] text-white/70 font-mono tabular-nums w-10 text-right">{Number(chromaBlend).toFixed(2)}</span>
                  </div>
                  <!-- Row 4: despill -->
                  <div class="flex items-center gap-2 w-full"
                       title={chromaAlgo === 'colorkey' ? 'Despill not meaningful for hard colorkey cuts.' : 'Remove coloured light bouncing onto the subject from the screen.'}>
                    <label class="flex items-center gap-1.5 rounded border border-[var(--border)] px-2 py-1 cursor-pointer shrink-0"
                           class:opacity-40={chromaAlgo === 'colorkey'}>
                      <input type="checkbox" bind:checked={chromaDespill}
                             disabled={chromaAlgo === 'colorkey'}
                             onchange={generateChromaPreview}
                             class="accent-[var(--accent)]"/>
                      <span class="text-[11px] text-white/70 font-medium">Despill</span>
                    </label>
                    <span class="text-[10px] uppercase tracking-wider text-white/40 font-semibold shrink-0 w-8">Mix</span>
                    <input type="range" min="0.00" max="1.00" step="0.05"
                           bind:value={chromaDespillMix}
                           oninput={generateChromaPreview}
                           disabled={!chromaDespill || chromaAlgo === 'colorkey'}
                           class="flex-1 accent-[var(--accent)] disabled:opacity-40"/>
                    <span class="text-[11px] text-white/70 font-mono tabular-nums w-10 text-right">{Number(chromaDespillMix).toFixed(2)}</span>
                  </div>
                  <!-- Row 5: upsample toggle -->
                  <div class="flex items-center gap-2 w-full">
                    <label class="flex items-center gap-1.5 rounded border border-[var(--border)] px-2 py-1 cursor-pointer"
                           title="Prepend format=yuv444p before the key. Helps noticeably on 4:2:0 sources where the chroma planes are blurry.">
                      <input type="checkbox" bind:checked={chromaUpsample}
                             onchange={generateChromaPreview}
                             class="accent-[var(--accent)]"/>
                      <span class="text-[11px] text-white/70 font-medium">Upsample chroma (yuv444p)</span>
                    </label>
                  </div>
                  <!-- Row 6: output container segmented -->
                  <div class="flex flex-wrap items-center gap-2 w-full">
                    <div class="inline-flex items-center rounded-md overflow-hidden border border-[var(--border)]">
                      <span class="px-2 text-[10px] uppercase tracking-wider text-white/40 font-semibold">Output</span>
                      <div class="w-px h-6 bg-[var(--border)]"></div>
                      <button onclick={() => chromaOutputFormat = 'mov_prores4444'}
                              title="MOV + ProRes 4444 — editorial standard, great alpha, big files."
                              class="px-2.5 py-1.5 text-[11px] font-semibold transition-colors
                                     {chromaOutputFormat === 'mov_prores4444' ? 'bg-[var(--accent)] text-white' : 'text-white/60 hover:bg-white/5'}">ProRes 4444</button>
                      <div class="w-px h-6 bg-[var(--border)]"></div>
                      <button onclick={() => chromaOutputFormat = 'mov_qtrle'}
                              title="MOV + QuickTime Animation (RLE) — lossless, huge files, very compatible."
                              class="px-2.5 py-1.5 text-[11px] font-semibold transition-colors
                                     {chromaOutputFormat === 'mov_qtrle' ? 'bg-[var(--accent)] text-white' : 'text-white/60 hover:bg-white/5'}">QtRLE</button>
                      <div class="w-px h-6 bg-[var(--border)]"></div>
                      <button onclick={() => chromaOutputFormat = 'webm_vp9'}
                              title="WebM + VP9 with yuva420p — browser-playable alpha video."
                              class="px-2.5 py-1.5 text-[11px] font-semibold transition-colors
                                     {chromaOutputFormat === 'webm_vp9' ? 'bg-[var(--accent)] text-white' : 'text-white/60 hover:bg-white/5'}">VP9+alpha</button>
                      <div class="w-px h-6 bg-[var(--border)]"></div>
                      <button onclick={() => chromaOutputFormat = 'png_sequence'}
                              title="PNG sequence — writes a sibling &lt;name&gt;_frames/ directory, one file per frame."
                              class="px-2.5 py-1.5 text-[11px] font-semibold transition-colors
                                     {chromaOutputFormat === 'png_sequence' ? 'bg-[var(--accent)] text-white' : 'text-white/60 hover:bg-white/5'}">PNG seq</button>
                      <div class="w-px h-6 bg-[var(--border)]"></div>
                      <button onclick={() => chromaOutputFormat = 'mkv_ffv1'}
                              title="MKV + FFV1 — lossless archival, smaller than QtRLE."
                              class="px-2.5 py-1.5 text-[11px] font-semibold transition-colors
                                     {chromaOutputFormat === 'mkv_ffv1' ? 'bg-[var(--accent)] text-white' : 'text-white/60 hover:bg-white/5'}">FFV1</button>
                    </div>
                  </div>
                  <!-- Row 7: preview + run -->
                  <div class="flex items-center gap-2 w-full">
                    <button
                      onclick={generateChromaPreview}
                      disabled={!selectedItem || selectedItem.mediaType !== 'video'}
                      class="px-3 py-1.5 rounded text-[12px] font-semibold border border-[var(--accent)] text-[var(--accent)] hover:bg-[color-mix(in_srgb,var(--accent)_12%,transparent)] transition-colors disabled:opacity-40"
                    >{chromaPreviewLoading ? 'Previewing…' : 'Preview frame'}</button>
                    <button
                      onclick={runChromaKey}
                      disabled={!selectedItem || selectedItem.mediaType !== 'video' || selectedItem.status === 'converting'}
                      class="px-3 py-1.5 rounded text-[12px] font-semibold bg-[var(--accent)] text-white hover:opacity-90 transition-opacity disabled:opacity-40"
                    >Run Chroma Key</button>
                  </div>
                  <!-- Preview image area with checkerboard backdrop. -->
                  <div class="w-full rounded border border-[var(--border)] overflow-hidden"
                       style="background-image:
                                linear-gradient(45deg, #333 25%, transparent 25%),
                                linear-gradient(-45deg, #333 25%, transparent 25%),
                                linear-gradient(45deg, transparent 75%, #333 75%),
                                linear-gradient(-45deg, transparent 75%, #333 75%);
                              background-size: 16px 16px;
                              background-position: 0 0, 0 8px, 8px -8px, -8px 0;
                              background-color: #1a1a1a;
                              min-height: 120px;">
                    {#if chromaPreviewUrl}
                      <img src={chromaPreviewUrl} alt="Chroma key preview"
                           class="w-full block"/>
                    {:else if chromaPreviewError}
                      <div class="p-3 text-[11px] text-red-400 font-mono">{chromaPreviewError}</div>
                    {:else}
                      <div class="p-3 text-[11px] text-white/30">No preview yet — click Preview frame after selecting a video.</div>
                    {/if}
                  </div>
                {:else if selectedOperation === 'loudness'}
                  <!-- Row 1: target preset + true-peak toggle -->
                  <div class="flex flex-wrap items-center gap-2 w-full">
                    <div class="flex items-center gap-1.5 rounded border border-[var(--border)] px-2 py-1">
                      <label class="text-[10px] uppercase tracking-wider text-white/40 font-semibold">Target</label>
                      <select bind:value={loudnessTarget}
                              class="bg-transparent text-[12px] text-white outline-none font-mono tabular-nums">
                        <option value="-23">-23 LUFS · Broadcast (EBU R128)</option>
                        <option value="-16">-16 LUFS · Streaming</option>
                        <option value="-14">-14 LUFS · Spotify / Apple Music</option>
                        <option value="custom">Custom…</option>
                      </select>
                    </div>
                    {#if loudnessTarget === 'custom'}
                      <div class="flex items-center gap-1.5 rounded border border-[var(--border)] px-2 py-1">
                        <label class="text-[10px] uppercase tracking-wider text-white/40 font-semibold">LUFS</label>
                        <input type="number" step="0.5" bind:value={loudnessTargetCustom}
                               class="w-16 bg-transparent text-[12px] text-white outline-none text-right font-mono tabular-nums"/>
                      </div>
                    {/if}
                    <label class="flex items-center gap-1.5 rounded border border-[var(--border)] px-2 py-1 cursor-pointer"
                           title="4× oversampling for accurate true-peak measurement. Slower but required for broadcast compliance.">
                      <input type="checkbox" bind:checked={loudnessTruePeak} class="accent-[var(--accent)]"/>
                      <span class="text-[11px] text-white/70 font-medium">True peak</span>
                    </label>
                  </div>
                  <!-- Row 2: run + results -->
                  <div class="w-full">
                    <button
                      onclick={runLoudness}
                      disabled={!selectedItem}
                      class="px-3 py-1.5 rounded text-[12px] font-semibold bg-[var(--accent)] text-white hover:opacity-90 transition-opacity disabled:opacity-40"
                    >Analyze</button>
                  </div>
                  <div class="w-full rounded border border-[var(--border)] bg-black/20 px-3 py-2 font-mono tabular-nums text-[12px] text-white/70">
                    {#if loudnessResult}
                      <div class="grid grid-cols-4 gap-2">
                        <div><span class="text-white/40 text-[10px] uppercase tracking-wider block">Integrated</span>{loudnessResult.I} LUFS</div>
                        <div><span class="text-white/40 text-[10px] uppercase tracking-wider block">LRA</span>{loudnessResult.LRA} LU</div>
                        <div><span class="text-white/40 text-[10px] uppercase tracking-wider block">True peak</span>{loudnessResult.TP} dBTP</div>
                        <div><span class="text-white/40 text-[10px] uppercase tracking-wider block">Threshold</span>{loudnessResult.threshold} LUFS</div>
                      </div>
                    {:else}
                      <span class="text-white/30">No results yet — Analyze to measure.</span>
                    {/if}
                  </div>
                {:else if selectedOperation === 'audio-norm'}
                  <!-- Row 1: mode segmented -->
                  <div class="flex flex-wrap items-center gap-2 w-full">
                    <div class="inline-flex items-center rounded-md overflow-hidden border border-[var(--border)]">
                      <span class="px-2 text-[10px] uppercase tracking-wider text-white/40 font-semibold">Mode</span>
                      <div class="w-px h-6 bg-[var(--border)]"></div>
                      <button onclick={() => audioNormMode = 'ebu'}
                              title="Two-pass EBU R128 loudnorm. Most accurate; preserves dynamics with linear=true."
                              class="px-2.5 py-1.5 text-[11px] font-semibold transition-colors
                                     {audioNormMode === 'ebu' ? 'bg-[var(--accent)] text-white' : 'text-white/60 hover:bg-white/5'}">EBU R128</button>
                      <div class="w-px h-6 bg-[var(--border)]"></div>
                      <button onclick={() => audioNormMode = 'peak'}
                              title="Scale peak sample to a dBFS ceiling. Fast, no perceptual weighting."
                              class="px-2.5 py-1.5 text-[11px] font-semibold transition-colors
                                     {audioNormMode === 'peak' ? 'bg-[var(--accent)] text-white' : 'text-white/60 hover:bg-white/5'}">Peak</button>
                      <div class="w-px h-6 bg-[var(--border)]"></div>
                      <button onclick={() => audioNormMode = 'rg'}
                              title="Write ReplayGain tags only. No sample changes. Players apply the gain on playback."
                              class="px-2.5 py-1.5 text-[11px] font-semibold transition-colors
                                     {audioNormMode === 'rg' ? 'bg-[var(--accent)] text-white' : 'text-white/60 hover:bg-white/5'}">ReplayGain tag</button>
                    </div>
                  </div>
                  <!-- Row 2: targets (EBU only shows LRA) -->
                  <div class="flex flex-wrap items-center gap-2 w-full">
                    <div class="flex items-center gap-1.5 rounded border border-[var(--border)] px-2 py-1">
                      <label class="text-[10px] uppercase tracking-wider text-white/40 font-semibold">{audioNormMode === 'peak' ? 'dBFS' : 'I'}</label>
                      <input type="number" step="0.5" bind:value={audioNormTargetI}
                             class="w-16 bg-transparent text-[12px] text-white outline-none text-right font-mono tabular-nums"/>
                    </div>
                    {#if audioNormMode === 'ebu'}
                      <div class="flex items-center gap-1.5 rounded border border-[var(--border)] px-2 py-1">
                        <label class="text-[10px] uppercase tracking-wider text-white/40 font-semibold">TP</label>
                        <input type="number" step="0.1" bind:value={audioNormTargetTP}
                               class="w-14 bg-transparent text-[12px] text-white outline-none text-right font-mono tabular-nums"/>
                      </div>
                      <div class="flex items-center gap-1.5 rounded border border-[var(--border)] px-2 py-1">
                        <label class="text-[10px] uppercase tracking-wider text-white/40 font-semibold">LRA</label>
                        <input type="number" step="0.5" bind:value={audioNormTargetLRA}
                               class="w-14 bg-transparent text-[12px] text-white outline-none text-right font-mono tabular-nums"/>
                      </div>
                      <label class="flex items-center gap-1.5 rounded border border-[var(--border)] px-2 py-1 cursor-pointer"
                             title="Two-pass with linear=true preserves dynamic range. Single-pass is dynamic compression.">
                        <input type="checkbox" bind:checked={audioNormLinear} class="accent-[var(--accent)]"/>
                        <span class="text-[11px] text-white/70 font-medium">Linear (preserve dynamics)</span>
                      </label>
                    {/if}
                  </div>
                  <!-- Row 3: run -->
                  <div class="w-full">
                    <button
                      onclick={runAudioNorm}
                      disabled={!selectedItem || selectedItem.status === 'converting'}
                      class="px-3 py-1.5 rounded text-[12px] font-semibold bg-[var(--accent)] text-white hover:opacity-90 transition-opacity disabled:opacity-40"
                    >Run Normalize</button>
                  </div>
                {:else if selectedOperation === 'cut-detect'}
                  <!-- Row 1: algo + threshold + min shot -->
                  <div class="flex flex-wrap items-center gap-2 w-full">
                    <div class="inline-flex items-center rounded-md overflow-hidden border border-[var(--border)]">
                      <span class="px-2 text-[10px] uppercase tracking-wider text-white/40 font-semibold">Algo</span>
                      <div class="w-px h-6 bg-[var(--border)]"></div>
                      <button onclick={() => cutDetectAlgo = 'scdet'}
                              title="FFmpeg scdet filter. Modern scoring, range ~5–15."
                              class="px-2.5 py-1.5 text-[11px] font-semibold transition-colors
                                     {cutDetectAlgo === 'scdet' ? 'bg-[var(--accent)] text-white' : 'text-white/60 hover:bg-white/5'}">scdet</button>
                      <div class="w-px h-6 bg-[var(--border)]"></div>
                      <button onclick={() => cutDetectAlgo = 'scene'}
                              title="Classic select='gt(scene,T)'. Range 0.2–0.5."
                              class="px-2.5 py-1.5 text-[11px] font-semibold transition-colors
                                     {cutDetectAlgo === 'scene' ? 'bg-[var(--accent)] text-white' : 'text-white/60 hover:bg-white/5'}">scene</button>
                    </div>
                    <div class="flex items-center gap-1.5 rounded border border-[var(--border)] px-2 py-1">
                      <label class="text-[10px] uppercase tracking-wider text-white/40 font-semibold">Threshold</label>
                      <input type="number" step="0.5" bind:value={cutDetectThreshold}
                             class="w-16 bg-transparent text-[12px] text-white outline-none text-right font-mono tabular-nums"/>
                    </div>
                    <div class="flex items-center gap-1.5 rounded border border-[var(--border)] px-2 py-1">
                      <label class="text-[10px] uppercase tracking-wider text-white/40 font-semibold">Min shot (s)</label>
                      <input type="number" step="0.1" bind:value={cutDetectMinShotS}
                             class="w-14 bg-transparent text-[12px] text-white outline-none text-right font-mono tabular-nums"/>
                    </div>
                  </div>
                  <!-- Row 2: run -->
                  <div class="w-full">
                    <button
                      onclick={runCutDetect}
                      disabled={!selectedItem}
                      class="px-3 py-1.5 rounded text-[12px] font-semibold bg-[var(--accent)] text-white hover:opacity-90 transition-opacity disabled:opacity-40"
                    >Detect Cuts</button>
                  </div>
                  <!-- Row 3: results -->
                  <div class="w-full rounded border border-[var(--border)] bg-black/20 px-3 py-2 font-mono tabular-nums text-[12px] max-h-40 overflow-y-auto">
                    {#if cutDetectResults.length}
                      {#each cutDetectResults as c, i}
                        <button class="w-full text-left py-0.5 px-1 hover:bg-white/5 text-white/70 flex justify-between">
                          <span>#{i + 1}</span>
                          <span>{c.time}s</span>
                          <span class="text-white/40">{c.score}</span>
                        </button>
                      {/each}
                    {:else}
                      <span class="text-white/30">No cuts detected yet — run to populate.</span>
                    {/if}
                  </div>
                {:else if selectedOperation === 'black-detect'}
                  <!-- Row 1: thresholds -->
                  <div class="flex flex-wrap items-center gap-2 w-full">
                    <div class="flex items-center gap-1.5 rounded border border-[var(--border)] px-2 py-1">
                      <label class="text-[10px] uppercase tracking-wider text-white/40 font-semibold">Min dur (s)</label>
                      <input type="number" step="0.05" bind:value={blackDetectMinDur}
                             class="w-16 bg-transparent text-[12px] text-white outline-none text-right font-mono tabular-nums"/>
                    </div>
                    <div class="flex items-center gap-1.5 rounded border border-[var(--border)] px-2 py-1"
                         title="pix_th: pixel luma threshold below which a pixel counts as black.">
                      <label class="text-[10px] uppercase tracking-wider text-white/40 font-semibold">Pix th</label>
                      <input type="number" step="0.01" min="0" max="1" bind:value={blackDetectPixTh}
                             class="w-16 bg-transparent text-[12px] text-white outline-none text-right font-mono tabular-nums"/>
                    </div>
                    <div class="flex items-center gap-1.5 rounded border border-[var(--border)] px-2 py-1"
                         title="pic_th: fraction of pixels below pix_th for a frame to count as black.">
                      <label class="text-[10px] uppercase tracking-wider text-white/40 font-semibold">Pic th</label>
                      <input type="number" step="0.01" min="0" max="1" bind:value={blackDetectPicTh}
                             class="w-16 bg-transparent text-[12px] text-white outline-none text-right font-mono tabular-nums"/>
                    </div>
                  </div>
                  <!-- Row 2: run -->
                  <div class="w-full">
                    <button
                      onclick={runBlackDetect}
                      disabled={!selectedItem}
                      class="px-3 py-1.5 rounded text-[12px] font-semibold bg-[var(--accent)] text-white hover:opacity-90 transition-opacity disabled:opacity-40"
                    >Detect Black</button>
                  </div>
                  <!-- Row 3: results -->
                  <div class="w-full rounded border border-[var(--border)] bg-black/20 px-3 py-2 font-mono tabular-nums text-[12px] max-h-40 overflow-y-auto">
                    {#if blackDetectResults.length}
                      {#each blackDetectResults as b, i}
                        <button class="w-full text-left py-0.5 px-1 hover:bg-white/5 text-white/70 grid grid-cols-4 gap-2">
                          <span>#{i + 1}</span>
                          <span>{b.start}s</span>
                          <span>{b.end}s</span>
                          <span class="text-white/40">{b.duration}s</span>
                        </button>
                      {/each}
                    {:else}
                      <span class="text-white/30">No black intervals detected yet.</span>
                    {/if}
                  </div>
                {:else if selectedOperation === 'vmaf'}
                  <!-- Row 1: file pickers -->
                  <div class="flex flex-wrap items-center gap-2 w-full">
                    <button onclick={() => vmafReferencePath
                              ? (vmafReferencePath = null)
                              : pickAuxFile((p) => vmafReferencePath = p, 'video/*')}
                            class="px-3 py-1.5 rounded text-[12px] font-medium border border-[var(--border)] text-[var(--text-secondary)] hover:text-[var(--text-primary)] hover:border-[var(--accent)] transition-colors">
                      {vmafReferencePath ? 'Clear reference' : 'Pick reference…'}
                    </button>
                    <button onclick={() => vmafDistortedPath
                              ? (vmafDistortedPath = null)
                              : pickAuxFile((p) => vmafDistortedPath = p, 'video/*')}
                            class="px-3 py-1.5 rounded text-[12px] font-medium border border-[var(--border)] text-[var(--text-secondary)] hover:text-[var(--text-primary)] hover:border-[var(--accent)] transition-colors">
                      {vmafDistortedPath ? 'Clear distorted' : 'Pick distorted…'}
                    </button>
                  </div>
                  <!-- Row 2: model + subsample -->
                  <div class="flex flex-wrap items-center gap-2 w-full">
                    <div class="flex items-center gap-1.5 rounded border border-[var(--border)] px-2 py-1">
                      <label class="text-[10px] uppercase tracking-wider text-white/40 font-semibold">Model</label>
                      <select bind:value={vmafModel}
                              class="bg-transparent text-[12px] text-white outline-none">
                        <option value="hd">HD · vmaf_v0.6.1</option>
                        <option value="4k">4K · vmaf_4k_v0.6.1</option>
                        <option value="phone">Phone</option>
                      </select>
                    </div>
                    <div class="flex items-center gap-1.5 rounded border border-[var(--border)] px-2 py-1"
                         title="Score every Nth frame. 1 = every frame (slowest, most accurate). 5 = 5× faster.">
                      <label class="text-[10px] uppercase tracking-wider text-white/40 font-semibold">Subsample</label>
                      <input type="number" step="1" min="1" bind:value={vmafSubsample}
                             class="w-12 bg-transparent text-[12px] text-white outline-none text-right font-mono tabular-nums"/>
                    </div>
                  </div>
                  <!-- Row 3: run + result -->
                  <div class="w-full">
                    <button
                      onclick={runVmaf}
                      disabled={!vmafReferencePath || !vmafDistortedPath}
                      class="px-3 py-1.5 rounded text-[12px] font-semibold bg-[var(--accent)] text-white hover:opacity-90 transition-opacity disabled:opacity-40"
                    >Run VMAF</button>
                  </div>
                  <div class="w-full rounded border border-[var(--border)] bg-black/20 px-3 py-2 font-mono tabular-nums text-[12px] text-white/70">
                    {#if vmafResult}
                      <div class="grid grid-cols-4 gap-2">
                        <div><span class="text-white/40 text-[10px] uppercase tracking-wider block">Mean</span>{vmafResult.mean}</div>
                        <div><span class="text-white/40 text-[10px] uppercase tracking-wider block">Harmonic</span>{vmafResult.harmonic_mean}</div>
                        <div><span class="text-white/40 text-[10px] uppercase tracking-wider block">Min</span>{vmafResult.min}</div>
                        <div><span class="text-white/40 text-[10px] uppercase tracking-wider block">Max</span>{vmafResult.max}</div>
                      </div>
                    {:else}
                      <span class="text-white/30">Drop a reference and distorted file, then Run.</span>
                    {/if}
                  </div>
                {:else if selectedOperation === 'framemd5'}
                  <!-- Row 1: stream picker + optional diff file -->
                  <div class="flex flex-wrap items-center gap-2 w-full">
                    <div class="inline-flex items-center rounded-md overflow-hidden border border-[var(--border)]">
                      <span class="px-2 text-[10px] uppercase tracking-wider text-white/40 font-semibold">Streams</span>
                      <div class="w-px h-6 bg-[var(--border)]"></div>
                      <button onclick={() => frameMd5Stream = 'video'}
                              class="px-2.5 py-1.5 text-[11px] font-semibold transition-colors
                                     {frameMd5Stream === 'video' ? 'bg-[var(--accent)] text-white' : 'text-white/60 hover:bg-white/5'}">Video</button>
                      <div class="w-px h-6 bg-[var(--border)]"></div>
                      <button onclick={() => frameMd5Stream = 'audio'}
                              class="px-2.5 py-1.5 text-[11px] font-semibold transition-colors
                                     {frameMd5Stream === 'audio' ? 'bg-[var(--accent)] text-white' : 'text-white/60 hover:bg-white/5'}">Audio</button>
                      <div class="w-px h-6 bg-[var(--border)]"></div>
                      <button onclick={() => frameMd5Stream = 'both'}
                              class="px-2.5 py-1.5 text-[11px] font-semibold transition-colors
                                     {frameMd5Stream === 'both' ? 'bg-[var(--accent)] text-white' : 'text-white/60 hover:bg-white/5'}">Both</button>
                    </div>
                    <button onclick={() => frameMd5DiffPath
                              ? (frameMd5DiffPath = null)
                              : pickAuxFile((p) => frameMd5DiffPath = p, 'audio/*,video/*')}
                            title="Optional second file. When set, first differing frame is highlighted."
                            class="px-3 py-1.5 rounded text-[12px] font-medium border border-[var(--border)] text-[var(--text-secondary)] hover:text-[var(--text-primary)] hover:border-[var(--accent)] transition-colors">
                      {frameMd5DiffPath ? 'Clear diff file' : 'Add diff file…'}
                    </button>
                  </div>
                  <!-- Row 2: run -->
                  <div class="w-full">
                    <button
                      onclick={runFrameMd5}
                      disabled={!selectedItem}
                      class="px-3 py-1.5 rounded text-[12px] font-semibold bg-[var(--accent)] text-white hover:opacity-90 transition-opacity disabled:opacity-40"
                    >{frameMd5DiffPath ? 'Run Diff' : 'Hash Frames'}</button>
                  </div>
                  <!-- Row 3: results -->
                  <div class="w-full rounded border border-[var(--border)] bg-black/20 px-3 py-2 font-mono tabular-nums text-[11px] text-white/70 max-h-40 overflow-y-auto">
                    {#if frameMd5Result}
                      {#if frameMd5Result.firstDivergence != null}
                        <div class="text-[var(--accent)] mb-1">First divergence at frame #{frameMd5Result.firstDivergence}</div>
                      {/if}
                      {#each frameMd5Result.hashes ?? [] as h}
                        <div class="flex gap-2"><span class="text-white/30 w-10 shrink-0">{h.idx}</span><span>{h.hash}</span></div>
                      {/each}
                    {:else}
                      <span class="text-white/30">No hashes yet — run to populate.</span>
                    {/if}
                  </div>
                {:else if selectedOperation === 'merge'}
                  <div class="flex flex-wrap items-center gap-2 w-full">
                    <button
                      onclick={() => {
                        if (selectedId && !mergeSelection.includes(selectedId)) {
                          mergeSelection = [...mergeSelection, selectedId];
                        }
                      }}
                      disabled={!selectedId || mergeSelection.includes(selectedId)}
                      class="px-3 py-1.5 rounded text-[12px] font-medium border border-[var(--border)] text-[var(--text-secondary)] hover:text-[var(--text-primary)] hover:border-[var(--accent)] transition-colors disabled:opacity-40"
                    >Add selected</button>
                    <button
                      onclick={() => mergeSelection = []}
                      disabled={mergeSelection.length === 0}
                      class="px-3 py-1.5 rounded text-[12px] font-medium border border-[var(--border)] text-[var(--text-secondary)] hover:text-[var(--text-primary)] hover:border-[var(--accent)] transition-colors disabled:opacity-40"
                    >Clear list</button>
                    <button
                      onclick={runMerge}
                      class="px-3 py-1.5 rounded text-[12px] font-semibold bg-[var(--accent)] text-white hover:opacity-90 transition-opacity disabled:opacity-40"
                      disabled={mergeSelection.length < 2}
                    >Run Merge</button>
                  </div>
                  <!-- Ordered list — top-to-bottom is concat order. -->
                  <div class="w-full rounded border border-[var(--border)] bg-black/20 px-2 py-1.5 max-h-48 overflow-y-auto">
                    {#if mergeSelection.length === 0}
                      <span class="text-white/30 text-[12px]">Select a file in the queue and click <em>Add selected</em>. Needs at least 2.</span>
                    {:else}
                      {#each mergeSelection as id, i (id)}
                        {@const q = queue.find(x => x.id === id)}
                        <div class="flex items-center gap-2 py-1">
                          <span class="text-white/40 font-mono tabular-nums text-[10px] w-5">{i + 1}.</span>
                          <span class="flex-1 truncate text-[12px] text-white/80">{q?.name ?? id}</span>
                          <button
                            onclick={() => {
                              if (i === 0) return;
                              const next = [...mergeSelection];
                              [next[i - 1], next[i]] = [next[i], next[i - 1]];
                              mergeSelection = next;
                            }}
                            disabled={i === 0}
                            class="px-1.5 py-0.5 text-[11px] rounded border border-[var(--border)] text-white/60 hover:text-white hover:border-[var(--accent)] disabled:opacity-30"
                            title="Move up"
                          >↑</button>
                          <button
                            onclick={() => {
                              if (i === mergeSelection.length - 1) return;
                              const next = [...mergeSelection];
                              [next[i], next[i + 1]] = [next[i + 1], next[i]];
                              mergeSelection = next;
                            }}
                            disabled={i === mergeSelection.length - 1}
                            class="px-1.5 py-0.5 text-[11px] rounded border border-[var(--border)] text-white/60 hover:text-white hover:border-[var(--accent)] disabled:opacity-30"
                            title="Move down"
                          >↓</button>
                          <button
                            onclick={() => mergeSelection = mergeSelection.filter(x => x !== id)}
                            class="px-1.5 py-0.5 text-[11px] rounded border border-[var(--border)] text-white/60 hover:text-red-400 hover:border-red-400"
                            title="Remove"
                          >×</button>
                        </div>
                      {/each}
                    {/if}
                  </div>
                {:else if selectedOperation === 'extract'}
                  {#each [
                    { id: 'video',    label: 'Video' },
                    { id: 'audio',    label: 'Audio' },
                    { id: 'subtitle', label: 'Subtitles' },
                    { id: 'all',      label: 'All streams' },
                  ] as m}
                    <button
                      onclick={() => extractMode = m.id}
                      class="px-3 py-1.5 rounded text-[12px] font-semibold border transition-colors
                             {extractMode === m.id
                               ? 'bg-[var(--accent)] border-[var(--accent)] text-white'
                               : 'border-[var(--border)] text-[var(--text-secondary)] hover:text-[var(--text-primary)] hover:border-[var(--accent)]'}"
                    >{m.label}</button>
                  {/each}
                  <button
                    onclick={runExtract}
                    disabled={!selectedItem || selectedItem.status === 'converting'}
                    class="px-3 py-1.5 rounded text-[12px] font-semibold bg-[var(--accent)] text-white hover:opacity-90 transition-opacity disabled:opacity-40"
                  >Run Extract</button>
                {:else if selectedOperation === 'subtitling'}
                  <!-- Unified Subtitling page — tabs for the three workflows
                       that the sidebar entries route into. -->
                  <div class="flex flex-col gap-3 w-full">
                    <!-- Tabs row -->
                    <div class="flex gap-1 border-b border-[var(--border)] w-full">
                      {#each [
                        { id: 'embed',    label: 'Embed / Burn-in' },
                        { id: 'generate', label: 'Generate (AI)' },
                        { id: 'analyze',  label: 'Analyze' },
                      ] as tab}
                        <button
                          onclick={() => subtitlingTab = tab.id}
                          class="px-3 py-1.5 text-[12px] font-medium -mb-px border-b-2 transition-colors
                                 {subtitlingTab === tab.id
                                   ? 'border-[var(--accent)] text-white'
                                   : 'border-transparent text-white/50 hover:text-white/80'}"
                        >{tab.label}</button>
                      {/each}
                    </div>

                    <!-- Tab bodies -->
                    {#if subtitlingTab === 'embed'}
                      <div class="flex flex-wrap gap-2">
                        <button class="px-3 py-1.5 rounded text-[12px] font-medium border border-[var(--border)] text-[var(--text-secondary)] hover:text-[var(--text-primary)] hover:border-[var(--accent)] transition-colors">Pick subtitle file…</button>
                        <button class="px-3 py-1.5 rounded text-[12px] font-medium border border-[var(--border)] text-[var(--text-secondary)] hover:text-[var(--text-primary)] hover:border-[var(--accent)] transition-colors">Burn-in</button>
                        <button class="px-3 py-1.5 rounded text-[12px] font-medium border border-[var(--border)] text-[var(--text-secondary)] hover:text-[var(--text-primary)] hover:border-[var(--accent)] transition-colors">Embed (soft)</button>
                        <button class="px-3 py-1.5 rounded text-[12px] font-medium border border-[var(--border)] text-[var(--text-secondary)] hover:text-[var(--text-primary)] hover:border-[var(--accent)] transition-colors">Style…</button>
                        <button class="px-3 py-1.5 rounded text-[12px] font-semibold bg-[var(--accent)] text-white hover:opacity-90 transition-opacity">Run Subtitling</button>
                      </div>
                    {:else if subtitlingTab === 'generate'}
                      <div class="flex flex-wrap gap-2">
                        <button class="px-3 py-1.5 rounded text-[12px] font-medium border border-[var(--border)] text-[var(--text-secondary)] hover:text-[var(--text-primary)] hover:border-[var(--accent)] transition-colors">Transcribe (Whisper)</button>
                        <button class="px-3 py-1.5 rounded text-[12px] font-medium border border-[var(--border)] text-[var(--text-secondary)] hover:text-[var(--text-primary)] hover:border-[var(--accent)] transition-colors">Translate SRT…</button>
                        <button class="px-3 py-1.5 rounded text-[12px] font-medium border border-[var(--border)] text-[var(--text-secondary)] hover:text-[var(--text-primary)] hover:border-[var(--accent)] transition-colors">Forced Align</button>
                        <button class="px-3 py-1.5 rounded text-[12px] font-medium border border-[var(--border)] text-[var(--text-secondary)] hover:text-[var(--text-primary)] hover:border-[var(--accent)] transition-colors">Diarize Speakers</button>
                        <button class="px-3 py-1.5 rounded text-[12px] font-medium border border-[var(--border)] text-[var(--text-secondary)] hover:text-[var(--text-primary)] hover:border-[var(--accent)] transition-colors">OCR Burned Subs</button>
                        <button class="px-3 py-1.5 rounded text-[12px] font-medium border border-[var(--border)] text-[var(--text-secondary)] hover:text-[var(--text-primary)] hover:border-[var(--accent)] transition-colors">Restore Punctuation</button>
                      </div>
                    {:else if subtitlingTab === 'analyze'}
                      <!-- Row 1: lint thresholds -->
                      <div class="flex flex-wrap items-center gap-2 w-full">
                        <div class="flex items-center gap-1.5 rounded border border-[var(--border)] px-2 py-1"
                             title="Characters-per-second reading speed ceiling. >21 is too fast to read comfortably.">
                          <label class="text-[10px] uppercase tracking-wider text-white/40 font-semibold">CPS max</label>
                          <input type="number" step="1" bind:value={subLintCpsMax}
                                 class="w-12 bg-transparent text-[12px] text-white outline-none text-right font-mono tabular-nums"/>
                        </div>
                        <div class="flex items-center gap-1.5 rounded border border-[var(--border)] px-2 py-1">
                          <label class="text-[10px] uppercase tracking-wider text-white/40 font-semibold">Min dur (ms)</label>
                          <input type="number" step="100" bind:value={subLintMinDurMs}
                                 class="w-16 bg-transparent text-[12px] text-white outline-none text-right font-mono tabular-nums"/>
                        </div>
                        <div class="flex items-center gap-1.5 rounded border border-[var(--border)] px-2 py-1">
                          <label class="text-[10px] uppercase tracking-wider text-white/40 font-semibold">Max dur (ms)</label>
                          <input type="number" step="100" bind:value={subLintMaxDurMs}
                                 class="w-16 bg-transparent text-[12px] text-white outline-none text-right font-mono tabular-nums"/>
                        </div>
                        <div class="flex items-center gap-1.5 rounded border border-[var(--border)] px-2 py-1">
                          <label class="text-[10px] uppercase tracking-wider text-white/40 font-semibold">Line max</label>
                          <input type="number" step="1" bind:value={subLintLineMaxChars}
                                 class="w-12 bg-transparent text-[12px] text-white outline-none text-right font-mono tabular-nums"/>
                        </div>
                        <div class="flex items-center gap-1.5 rounded border border-[var(--border)] px-2 py-1">
                          <label class="text-[10px] uppercase tracking-wider text-white/40 font-semibold">Lines max</label>
                          <input type="number" step="1" bind:value={subLintMaxLines}
                                 class="w-10 bg-transparent text-[12px] text-white outline-none text-right font-mono tabular-nums"/>
                        </div>
                      </div>
                      <!-- Row 2: actions -->
                      <div class="flex flex-wrap items-center gap-2 w-full">
                        <button
                          onclick={runSubLint}
                          disabled={!selectedItem}
                          class="px-3 py-1.5 rounded text-[12px] font-semibold bg-[var(--accent)] text-white hover:opacity-90 transition-opacity disabled:opacity-40"
                        >Run Lint</button>
                        <button onclick={() => subDiffReferencePath
                                  ? (subDiffReferencePath = null)
                                  : pickAuxFile((p) => subDiffReferencePath = p, '.srt,.vtt,.ass,.ssa')}
                                title="Optional reference subtitle for a diff. Without one, Lint runs on the selected file."
                                class="px-3 py-1.5 rounded text-[12px] font-medium border border-[var(--border)] text-[var(--text-secondary)] hover:text-[var(--text-primary)] hover:border-[var(--accent)] transition-colors">
                          {subDiffReferencePath ? 'Clear diff reference' : 'Pick diff reference…'}
                        </button>
                        {#if subDiffReferencePath}
                          <button
                            onclick={runSubDiff}
                            disabled={!selectedItem}
                            class="px-3 py-1.5 rounded text-[12px] font-semibold bg-[var(--accent)] text-white hover:opacity-90 transition-opacity disabled:opacity-40"
                          >Run Diff</button>
                        {/if}
                        <button
                          onclick={runSubProbe}
                          disabled={!selectedItem}
                          class="px-3 py-1.5 rounded text-[12px] font-medium border border-[var(--border)] text-[var(--text-secondary)] hover:text-[var(--text-primary)] hover:border-[var(--accent)] transition-colors disabled:opacity-40"
                          title="Probe the currently selected video for subtitle streams (ffprobe).">Detect tracks</button>
                      </div>

                      <!-- Results: lint issues / diff lines / detected tracks -->
                      {#if subLintResults}
                        <div class="w-full rounded border border-[var(--border)] bg-black/20 px-3 py-2 text-[11px] font-mono max-h-48 overflow-y-auto">
                          {#if subLintResults.length === 0}
                            <span class="text-white/30">No lint issues.</span>
                          {:else}
                            {#each subLintResults as issue}
                              <div class="flex gap-2 py-0.5">
                                <span class="text-white/40 w-20 shrink-0">{issue.time}</span>
                                <span class="text-[var(--accent)] w-28 shrink-0">{issue.kind}</span>
                                <span class="text-white/70">{issue.message}</span>
                              </div>
                            {/each}
                          {/if}
                        </div>
                      {/if}
                      {#if subDiffResults}
                        <div class="w-full rounded border border-[var(--border)] bg-black/20 px-3 py-2 text-[11px] font-mono max-h-48 overflow-y-auto">
                          {#each subDiffResults as d}
                            <div class="flex gap-2 py-0.5
                                        {d.kind === 'add' ? 'text-emerald-400' : d.kind === 'del' ? 'text-red-400' : 'text-white/50'}">
                              <span class="w-4 shrink-0">{d.kind === 'add' ? '+' : d.kind === 'del' ? '-' : ' '}</span>
                              <span class="truncate">{d.left ?? d.right ?? ''}</span>
                            </div>
                          {/each}
                        </div>
                      {/if}
                      {#if subProbeResults}
                        <div class="w-full rounded border border-[var(--border)] bg-black/20 px-3 py-2 text-[11px] font-mono max-h-40 overflow-y-auto">
                          {#if subProbeResults.length === 0}
                            <span class="text-white/30">No subtitle streams found.</span>
                          {:else}
                            {#each subProbeResults as s}
                              <div class="flex gap-2 py-0.5">
                                <span class="text-white/40 w-10 shrink-0">#{s.index}</span>
                                <span class="text-white/70 w-24 shrink-0">{s.codec}</span>
                                <span class="text-white/60 w-20 shrink-0">{s.language ?? '—'}</span>
                                <span class="text-white/50 truncate">{s.title ?? ''}</span>
                              </div>
                            {/each}
                          {/if}
                        </div>
                      {/if}
                    {/if}
                  </div>
                {:else if selectedOperation === 'video-inserts'}
                  <button class="px-3 py-1.5 rounded text-[12px] font-medium border border-[var(--border)] text-[var(--text-secondary)] hover:text-[var(--text-primary)] hover:border-[var(--accent)] transition-colors">Set insert point</button>
                  <button class="px-3 py-1.5 rounded text-[12px] font-medium border border-[var(--border)] text-[var(--text-secondary)] hover:text-[var(--text-primary)] hover:border-[var(--accent)] transition-colors">Pick insert clip…</button>
                  <button class="px-3 py-1.5 rounded text-[12px] font-medium border border-[var(--border)] text-[var(--text-secondary)] hover:text-[var(--text-primary)] hover:border-[var(--accent)] transition-colors">Trim insert</button>
                  <button class="px-3 py-1.5 rounded text-[12px] font-semibold bg-[var(--accent)] text-white hover:opacity-90 transition-opacity">Run Insert</button>
                {/if}
              </div>
            </div>
          </div>
        {/if}

        <!-- ── NON-VIDEO content: key block remounts on each selection ── -->
        {#if !operationsMode}
        {#key selectedId}
          {#if selectedItem?.kind === 'folder'}
            <!-- Batch folder configuration panel — UI only. Wiring up rename
                 and output-routing logic happens in a follow-up. -->
            {@const bf = selectedItem}
            {@const opts = bf.batchOptions}
            <div class="w-full h-full flex select-none">
              <!-- ── Drop zone (left rail) — hit-tested via data-folder-drop ── -->
              <div
                data-folder-drop={bf.id}
                role="region"
                aria-label="Drop files into this proxy node"
                class="shrink-0 w-44 m-4 mr-0 rounded-2xl border-2 border-dashed
                       flex flex-col items-center justify-center gap-3 p-4 text-center
                       transition-all duration-150
                       {draggingFileId
                         ? 'border-[var(--accent)] bg-[var(--accent)]/15 text-[var(--accent)]'
                         : 'border-white/15 bg-white/[0.04] text-white/45'}"
              >
                <svg width="32" height="32" viewBox="0 0 24 24" fill="none" stroke="currentColor"
                     stroke-width="1.4" stroke-linecap="round" stroke-linejoin="round">
                  <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"/>
                  <polyline points="17 8 12 3 7 8"/>
                  <line x1="12" y1="3" x2="12" y2="15"/>
                </svg>
                <p class="text-[12px] font-medium leading-tight">Drop files here</p>
                <p class="text-[10px] leading-snug opacity-70">
                  Drag from the queue on the left into this zone to add them to <span class="font-semibold">{bf.name}</span>.
                </p>
              </div>

              <div class="flex-1 min-w-0 overflow-y-auto px-10 py-8">
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
                               hover:border-[var(--accent)] hover:text-[var(--accent)] transition-colors"
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
              {selectedId}
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
             style="background:var(--surface-raised)">
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
              placeholder="Filter tools…"
              class="w-full pl-7 pr-6 py-1 text-[11px] rounded border border-[var(--border)]
                     bg-[color:color-mix(in_srgb,#000_35%,var(--surface-raised))]
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
          <button
            onclick={() => { globalOutputFormat = null; }}
            data-tooltip="Back to the format picker grid."
            data-back-button
            class="px-3 py-1.5 rounded text-[11px] font-semibold bg-[var(--accent)] text-white
                   hover:opacity-90 transition-opacity shrink-0"
          >← Back</button>
        {:else}
          <button
            onclick={() => { globalOutputFormat = null; }}
            data-tooltip="Target output format for every queued file. Click to pick a new format — returns to the format picker grid below."
            class="btn-bevel px-3 py-1 text-[13px] flex items-center gap-1.5 shrink-0"
          >
            Output
            <svg width="8" height="5" viewBox="0 0 8 5" fill="none" stroke="currentColor"
                 stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"
                 class="shrink-0">
              <path d="M1 1l3 3 3-3"/>
            </svg>
          </button>
        {/if}

        <!-- Presets selector — always visible; filtered to active category when one is selected -->
        <PresetManager
          bind:this={presetManagerEl}
          bind:imageOptions
          bind:videoOptions
          bind:audioOptions
          bind:globalOutputFormat
          {activeOutputCategory}
          {setStatus}
        />
      </div>

      <!-- Active-format page title: large, centered, only when a format is
           picked. Replaces the cramped extension chip that used to live in
           the header button — much clearer signal of "which page am I on". -->
      {#if globalOutputFormat}
        {@const _fmt = FORMAT_GROUPS.find(g => g.fmts.some(f => f.id === globalOutputFormat))?.fmts.find(f => f.id === globalOutputFormat)}
        <div class="flex items-center justify-center px-3 py-3 shrink-0 border-b border-[var(--border)]">
          <h2 class="text-[20px] font-semibold text-white/85">
            {_fmt?.label ?? globalOutputFormat.toUpperCase()}
          </h2>
        </div>
      {/if}

      <!-- Search — filters the format/tool grid below. Only visible on the
           picker page (when globalOutputFormat is null) so it doesn't clutter
           the options pages. -->
      {#if !globalOutputFormat}
        <div class="shrink-0 px-2 py-1.5 border-b border-[var(--border)]"
             style="background:var(--surface-raised)">
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
              placeholder="Filter formats & tools — try mp3, prores, conform…"
              class="w-full pl-7 pr-6 py-1 text-[11px] rounded border border-[var(--border)]
                     bg-[color:color-mix(in_srgb,#000_35%,var(--surface-raised))]
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
      <div class="flex-1 min-h-0 overflow-y-auto p-4">
        {#if !globalOutputFormat}
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
                    {@const _fmts = group.fmts.filter(f => (!f.todo || (f.preview && settings.showDevFeatures) || import.meta.env.DEV) && matchesSearch(f))}
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
                          {@const entryCat = f.cat ?? group.cat}
                          {@const incompatible = compatibleOutputCats !== null && !compatibleOutputCats.includes(entryCat === 'codec' ? 'video' : entryCat)}
                          <button
                            onclick={() => {
                              if (incompatible) return;
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
                            data-tooltip={incompatible ? `${(f.label ?? f.id).toUpperCase()} — incompatible with current queue contents` : (group.cat === 'codec' ? `${f.label} — encode as ${f.codec} in ${f.ext.toUpperCase()}` : `Convert to ${(f.label ?? f.id).toUpperCase()} — ${group.label.toLowerCase()} output`)}
                            class="px-2 py-0.5 rounded text-[11px] font-mono border transition-colors
                                   {incompatible ? 'opacity-25 cursor-default' : ''}
                                   {f.todo
                                     ? 'border-green-900 text-green-400 hover:border-green-600 hover:bg-green-950'
                                     : 'border-[var(--border)] text-[var(--text-primary)] hover:border-[var(--accent)] hover:text-[var(--accent)]'}"
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
                    {@const _fmts = group.fmts.filter(f => (!f.todo || (f.preview && settings.showDevFeatures) || import.meta.env.DEV || isOpsGroup || f.ops) && matchesSearch(f))}
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
                          {@const incompatible = compatibleOutputCats !== null && !compatibleOutputCats.includes(group.cat) && !isOpsEntry}
                          <button
                            onclick={() => {
                              if (incompatible) return;
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
                            data-tooltip={incompatible ? `${(f.label ?? f.id).toUpperCase()} — incompatible with current queue contents` : (isOpsEntry ? `${(f.label ?? f.id)} — open operations mode` : `Convert to ${(f.label ?? f.id).toUpperCase()} — ${group.label.toLowerCase()} output`)}
                            class="px-2 py-0.5 rounded text-[11px] font-mono border transition-colors
                                   {incompatible ? 'opacity-25 cursor-default' : ''}
                                   {f.todo && !isOpsEntry
                                     ? 'border-green-900 text-green-400 hover:border-green-600 hover:bg-green-950'
                                     : 'border-[var(--border)] text-[var(--text-primary)] hover:border-[var(--accent)] hover:text-[var(--accent)]'}"
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
                  {#each fileGroups.filter(g => !g.todo || import.meta.env.DEV || settings.showDevFeatures) as group (group.cat)}
                    {@const _fmts = group.fmts.filter(f => (!f.todo || (f.preview && settings.showDevFeatures) || import.meta.env.DEV) && matchesSearch(f))}
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
                          {@const incompatible = compatibleOutputCats !== null && !compatibleOutputCats.includes(group.cat)}
                          <button
                            onclick={() => { if (!incompatible) globalOutputFormat = f.id; }}
                            data-tooltip={incompatible ? `${(f.label ?? f.id).toUpperCase()} — incompatible with current queue contents` : `Convert to ${(f.label ?? f.id).toUpperCase()} — ${group.label.toLowerCase()} output`}
                            class="px-2 py-0.5 rounded text-[11px] font-mono border transition-colors
                                   {incompatible ? 'opacity-25 cursor-default' : ''}
                                   {f.todo
                                     ? 'border-green-900 text-green-400 hover:border-green-600 hover:bg-green-950'
                                     : 'border-[var(--border)] text-[var(--text-primary)] hover:border-[var(--accent)] hover:text-[var(--accent)]'}"
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
        {:else if activeOutputCategory === 'image'}
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

      <!-- ── Bottom footer: hint text + zoom controls ────────────────────── -->
      <div class="shrink-0 border-t border-[var(--border)] flex flex-col gap-2 px-3 pt-2.5 pb-1"
           style="background:color-mix(in srgb, var(--surface-raised) 60%, #000 40%)">

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
                   {zoom.level !== 1.0 ? 'text-[var(--accent)]' : ''}"
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
      {/if}

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
          <!-- Logo placeholder -->
          <div class="w-16 h-16 rounded-2xl border border-[var(--border)] flex items-center justify-center"
               style="background:color-mix(in srgb, var(--accent) 12%, #000)">
            <span class="text-2xl font-bold" style="color:var(--accent)">F</span>
          </div>
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
            Fade is a fast, offline media converter built for people who know what they're doing.
            No subscriptions, no cloud, no limits — just ffmpeg with a decent interface.
          </p>
          <p>
            Part of the <strong class="text-[var(--text-primary)]">Libre</strong> family of professional tools
            by <!-- svelte-ignore a11y_missing_attribute -->
            <a onclick={(e) => { e.stopPropagation(); invoke('open_url', { url: 'https://irontreesoftware.com' }); }}
               class="text-[var(--text-primary)] underline decoration-white/20 hover:decoration-white/60 cursor-pointer transition-all">
              Iron Tree Software
            </a> — built to own your workflow.
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
