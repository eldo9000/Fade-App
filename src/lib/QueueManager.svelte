<script>
  import { invoke, convertFileSrc } from '@tauri-apps/api/core';
  import { listen } from '@tauri-apps/api/event';
  import { mediaTypeFor } from './utils.js';
  import Queue from './Queue.svelte';

  // ── Props API (spec-required set) ─────────────────────────────────────────
  let {
    // $bindable outputs — App.svelte reads these
    selectedItem     = $bindable(null),
    selectedMediaType = $bindable(null),
    queue            = $bindable([]),
    // Preview pipeline state — App.svelte binds these to drive Timeline / video element
    liveSrc               = $bindable(null),
    selectedDuration      = $bindable(null),
    selectedWidth         = $bindable(null),
    selectedHeight        = $bindable(null),
    previewLoading        = $bindable(false),
    tlMediaReady          = $bindable(false),
    tlWaveformReady       = $bindable(false),
    tlSpectrogramReady    = $bindable(false),
    tlFilmstripReady      = $bindable(false),
    cachedWaveformForTimeline  = $bindable(null),
    cachedFilmstripForTimeline = $bindable(null),
    // Selection state — App.svelte needs selectedIds.size for Deselect button
    selectedIds     = $bindable(new Set()),
    // Drag state — App.svelte reads draggingFileId for folder drop hover logic
    draggingFileId  = $bindable(null),
    // Plain input props
    visibleQueue        = null,
    setStatus           = null,
    compatibleTypes     = [],
    onSelectionChange   = null,
    // Queue display props forwarded to Queue
    compact         = false,
    showExtColumn   = true,
  } = $props();

  // ── Internal selection state ───────────────────────────────────────────────
  let selectedId     = $state(null);
  let selectAnchorId = $state(null);

  // Keep $bindable selectedItem / selectedMediaType in sync with internal selectedId
  $effect(() => {
    const found = queue.find(q => q.id === selectedId) ?? null;
    selectedItem      = found;
    selectedMediaType = found?.mediaType ?? null;
  });

  // ── Batch folder counter ───────────────────────────────────────────────────
  let batchFolderCounter = $state(0);

  // ── Pre-load cache ─────────────────────────────────────────────────────────
  let preloadCache = new Map(); // id → { waveform?, filmstripFrames? }
  let _bgBusy = false;

  // ── Async pipeline generation counter — plain number, NOT $state ──────────
  // Incremented on every selection; runLoadPipeline bails if it changes mid-flight.
  let _loadGen = 0;

  // ── Heavy-file thresholds (mirrors App.svelte constants) ──────────────────
  const HEAVY_FILE_BYTES    = 500 * 1024 * 1024; // 500 MB
  const HEAVY_DURATION_SECS = 30 * 60;           // 30 min

  function isHeavyItem(item) {
    const info = item?.info;
    if (!info) return false;
    if ((info.file_size    ?? 0) > HEAVY_FILE_BYTES)    return true;
    if ((info.duration_secs ?? 0) > HEAVY_DURATION_SECS) return true;
    return false;
  }

  /** Yield to browser for one frame — returns a Promise that resolves after paint */
  function frameYield(ms = 50) { return new Promise(r => setTimeout(r, ms)); }

  // ── Sequential load pipeline ───────────────────────────────────────────────
  //
  //  handleSelect (synchronous):
  //    increments _loadGen, updates selectedId / selectedIds
  //    calls onSelectionChange(newItem, { isMedia }) so App.svelte can clear
  //    preview state and run vizExpanded logic
  //
  //  runLoadPipeline (async, each stage awaits previous):
  //    Step 1  (50ms yield)         — let browser paint cleared state
  //    Step 2  get_file_info        — fast ffprobe metadata → selectedDuration etc.
  //    Step 3  liveSrc              — set video/image src → browser decode starts
  //    Step 4  (frameYield) + tlMediaReady   — Timeline creates Audio / connects <video>
  //    Step 5  tlWaveformReady      — unlock waveform (ffmpeg, medium cost)
  //    Step 6  tlSpectrogramReady   — unlock spectrogram (ffmpeg, heaviest)
  //    Step 7  tlFilmstripReady     — unlock filmstrip (background, lowest priority)
  //
  //  Every await is followed by a gen check before mutating state.

  async function runLoadPipeline(gen, newItem) {
    const isMedia = newItem && ['video', 'audio', 'image'].includes(newItem.mediaType);
    if (!newItem || !isMedia) return;

    const stale = () => _loadGen !== gen;
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
      // Audio — no visual preview, clear loading immediately
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
    // Small beat so waveform invoke dispatches before spectrogram starts
    await frameYield(100);
    if (stale()) return;
    tlSpectrogramReady = true;

    // ── Step 7: unlock filmstrip (background, lowest priority) ──
    // Skip the long delay if filmstrip is already in cache (instant display).
    if (!cachedFilmstripForTimeline) {
      await frameYield(800);
    }
    if (stale()) return;
    tlFilmstripReady = true;
  }

  // ── Background preloader ───────────────────────────────────────────────────
  async function _bgPreloadNext() {
    if (_bgBusy) return;

    const nextItem = queue.find(item =>
      item.id !== selectedId &&
      ['video', 'audio'].includes(item.mediaType) &&
      !preloadCache.has(item.id)
    );
    if (!nextItem) return;

    _bgBusy = true;
    const cached = {};
    preloadCache.set(nextItem.id, cached);

    try {
      const data = await invoke('get_waveform', { path: nextItem.path, draft: isHeavyItem(nextItem) });
      cached.waveform = data;
      preloadCache.set(nextItem.id, cached);
    } catch { /* non-fatal */ }

    if (nextItem.mediaType === 'video') {
      const dur = nextItem.info?.duration_secs ?? null;
      if (dur) {
        const draft = isHeavyItem(nextItem);
        cached.filmstripFrames = new Array(20).fill(null);
        preloadCache.set(nextItem.id, cached);
        invoke('get_filmstrip', {
          path: nextItem.path, id: nextItem.id + '-bg',
          count: 20, duration: dur, draft,
        }).catch(() => {});
      }
    }

    _bgBusy = false;
    setTimeout(_bgPreloadNext, 100);
  }

  // ── Lifecycle: bg filmstrip listener ──────────────────────────────────────
  $effect(() => {
    let unlisten = null;
    listen('filmstrip-frame', (ev) => {
      const { id, index, data } = ev.payload;
      if (!id.endsWith('-bg')) return;
      const realId = id.slice(0, -3);
      const cached = preloadCache.get(realId);
      if (!cached) return;
      if (!cached.filmstripFrames) cached.filmstripFrames = new Array(20).fill(null);
      cached.filmstripFrames[index] = data;
      preloadCache.set(realId, cached);
    }).then(fn => { unlisten = fn; });
    return () => { unlisten?.(); };
  });

  // ── Queue mutations ────────────────────────────────────────────────────────

  /** Add file paths to the queue. */
  export function addFiles(paths) {
    for (const path of paths) {
      const name = path.split('/').pop() ?? path;
      const ext  = name.includes('.') ? name.split('.').pop().toLowerCase() : '';
      const mt   = mediaTypeFor(ext);
      const id   = crypto.randomUUID();
      const item = { id, kind: 'file', parentId: null, path, name, ext, mediaType: mt, status: 'pending', percent: 0, info: null };
      queue.push(item);
      if (['video', 'audio', 'image'].includes(mt)) {
        invoke('get_file_info', { path }).then(info => {
          const q = queue.find(q => q.id === id);
          if (q) q.info = info;
        }).catch(() => {});
      }
    }
    setTimeout(_bgPreloadNext, 500);
  }

  /** Remove a queue item by id. Advances selection if it was the selected item. */
  export function removeItem(id) {
    queue = queue.filter(q => q.id !== id);
    if (selectedId === id) handleSelect(queue.length > 0 ? queue[0].id : null);
  }

  /** Move a queued file into a batch folder (null = move to root). */
  function moveItemToFolder(itemId, targetFolderId) {
    if (!itemId || itemId === targetFolderId) return;
    const idx = queue.findIndex(q => q.id === itemId);
    if (idx === -1) return;
    const item = queue[idx];
    if (item.kind !== 'file') return;
    if (targetFolderId) {
      const target = queue.find(q => q.id === targetFolderId);
      if (!target || target.kind !== 'folder') return;
    }
    if (item.parentId === targetFolderId) return;
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

  /** Add a batch (proxy) folder node and select it. */
  export function addBatchFolder() {
    batchFolderCounter += 1;
    const id = crypto.randomUUID();
    queue.push({
      id,
      kind: 'folder',
      name: `Proxy Node ${batchFolderCounter}`,
      expanded: true,
      status: 'pending',
      batchOptions: {
        renameMode: 'suffix',
        renameToken: '_proxy',
        renamePattern: '{name}_{n}',
        outputMode: 'mirror',
        outputRoot: '',
        preserveStructure: true,
      },
    });
    handleSelect(id);
  }

  /** Clear all queue items and reset selection. */
  export function clearQueue() {
    queue          = [];
    selectedId     = null;
    selectedIds    = new Set();
    selectAnchorId = null;
    setStatus?.('', 'info');
  }

  /** Clear the preload cache (e.g. after settings change). */
  export function clearPreloadCache() {
    preloadCache.clear();
    cachedWaveformForTimeline  = null;
    cachedFilmstripForTimeline = null;
  }

  // ── Cancel ─────────────────────────────────────────────────────────────────

  async function cancelJob(id) {
    try {
      await invoke('cancel_job', { jobId: id });
    } catch { /* non-fatal */ }
  }

  // ── Selection logic ────────────────────────────────────────────────────────

  /** Ids the user is allowed to select. Folders are always selectable. */
  function _selectableIds() {
    const src = visibleQueue ?? queue;
    return src
      .filter(q => q.kind === 'folder' || compatibleTypes.length === 0 || compatibleTypes.includes(q.mediaType))
      .map(q => q.id);
  }

  /**
   * Primary selection handler. Exported so App.svelte can call it directly
   * (e.g. from the incompatible-type $effect and the Deselect button).
   */
  export function handleSelect(id, mods = null) {
    // ── Modifier-aware multi-select ──
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
          selectedIds    = new Set([id]);
          selectAnchorId = id;
        }
      } else if (mods.meta || mods.ctrl) {
        const next = new Set(selectedIds);
        if (next.has(id)) next.delete(id); else next.add(id);
        selectedIds    = next;
        selectAnchorId = id;
      } else {
        selectedIds    = new Set([id]);
        selectAnchorId = id;
      }
    } else if (id) {
      selectedIds    = new Set([id]);
      selectAnchorId = id;
    } else {
      selectedIds    = new Set();
      selectAnchorId = null;
    }

    const gen     = ++_loadGen;  // cancel any in-flight pipeline
    const newItem = id ? queue.find(q => q.id === id) : null;
    const isMedia = !!(newItem && ['video', 'audio', 'image'].includes(newItem.mediaType));

    selectedId = id ?? null;

    // Serve pre-loaded data to Timeline immediately (avoids re-invoking ffmpeg)
    const _cached = preloadCache.get(id ?? '');
    cachedWaveformForTimeline  = _cached?.waveform        ?? null;
    cachedFilmstripForTimeline = _cached?.filmstripFrames ?? null;

    // Notify App.svelte: clear preview state, run vizExpanded logic, etc.
    onSelectionChange?.(newItem, { isMedia });

    // ── Async pipeline: stages run sequentially after browser paints ──
    runLoadPipeline(gen, newItem);
  }

  /** Convenience: clear the full multi-selection and deselect. */
  export function deselectAll() {
    selectedIds    = new Set();
    selectAnchorId = null;
    handleSelect(null);
  }
</script>

<Queue
  queue={visibleQueue ?? queue}
  {selectedId}
  {selectedIds}
  onselect={handleSelect}
  onremove={(id) => removeItem(id)}
  oncancel={(id) => cancelJob(id)}
  ontogglefolder={toggleFolderExpanded}
  onmovetofolder={moveItemToFolder}
  ondragstartfile={(id) => { draggingFileId = id; }}
  ondragendfile={() => { draggingFileId = null; }}
  disableHoverInfo={selectedItem?.kind === 'folder'}
  {compatibleTypes}
  {compact}
  {showExtColumn}
/>
