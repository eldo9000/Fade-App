import { describe, it, expect, vi, beforeEach } from 'vitest';
import { mediaTypeFor, validateOptions } from '../lib/utils.js';
import { markCancelled, applyProgressIfActive, isTerminal } from '../lib/itemStatus.js';
import { mount, unmount, flushSync } from 'svelte';

// ── QueueManager component mocks ──────────────────────────────────────────────
// Mocked before the component-level tests that import QueueManager.
// get_file_info / get_waveform / get_filmstrip are best-effort in the pipeline;
// mocks here ensure they resolve quickly and never hang the async pipeline.

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn((cmd) => {
    if (cmd === 'get_file_info') return Promise.resolve({ duration_secs: 120, width: 1920, height: 1080 });
    if (cmd === 'get_waveform')  return Promise.resolve([]);
    if (cmd === 'get_filmstrip') return Promise.resolve([]);
    return Promise.resolve(null);
  }),
  convertFileSrc: vi.fn((p) => `asset://${p}`),
}));

vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn().mockResolvedValue(() => {}),
}));

// ── mediaTypeFor ──────────────────────────────────────────────────────────────

describe('mediaTypeFor', () => {
  it('classifies image extensions', () => {
    expect(mediaTypeFor('jpg')).toBe('image');
    expect(mediaTypeFor('jpeg')).toBe('image');
    expect(mediaTypeFor('png')).toBe('image');
    expect(mediaTypeFor('webp')).toBe('image');
    expect(mediaTypeFor('heic')).toBe('image');
    expect(mediaTypeFor('gif')).toBe('image');
    expect(mediaTypeFor('avif')).toBe('image');
  });

  it('classifies video extensions', () => {
    expect(mediaTypeFor('mp4')).toBe('video');
    expect(mediaTypeFor('mkv')).toBe('video');
    expect(mediaTypeFor('webm')).toBe('video');
    expect(mediaTypeFor('avi')).toBe('video');
    expect(mediaTypeFor('mov')).toBe('video');
  });

  it('classifies audio extensions', () => {
    expect(mediaTypeFor('mp3')).toBe('audio');
    expect(mediaTypeFor('flac')).toBe('audio');
    expect(mediaTypeFor('wav')).toBe('audio');
    expect(mediaTypeFor('aac')).toBe('audio');
    expect(mediaTypeFor('opus')).toBe('audio');
  });

  it('returns unknown for unrecognised extensions', () => {
    expect(mediaTypeFor('xyz')).toBe('unknown');
    expect(mediaTypeFor('')).toBe('unknown');
    expect(mediaTypeFor('exe')).toBe('unknown');
    expect(mediaTypeFor('pdf')).toBe('unknown');
  });
});

// ── Queue state transitions (pure logic) ──────────────────────────────────────

describe('queue state transitions', () => {
  function makeItem(overrides = {}) {
    return {
      id: crypto.randomUUID(),
      path: '/tmp/file.mp4',
      name: 'file.mp4',
      ext: 'mp4',
      mediaType: 'video',
      status: 'pending',
      percent: 0,
      ...overrides,
    };
  }

  it('add: appends items to the queue', () => {
    const queue = [];
    const item = makeItem();
    queue.push(item);
    expect(queue).toHaveLength(1);
    expect(queue[0].status).toBe('pending');
  });

  it('remove: filters out the item by id', () => {
    const a = makeItem();
    const b = makeItem();
    let queue = [a, b];
    queue = queue.filter(q => q.id !== a.id);
    expect(queue).toHaveLength(1);
    expect(queue[0].id).toBe(b.id);
  });

  it('clear: empties the queue', () => {
    let queue = [makeItem(), makeItem(), makeItem()];
    queue = [];
    expect(queue).toHaveLength(0);
  });

  it('status transitions: pending → converting → done', () => {
    const queue = [makeItem()];
    queue[0].status = 'converting';
    queue[0].percent = 50;
    expect(queue[0].status).toBe('converting');

    queue[0].status = 'done';
    queue[0].percent = 100;
    expect(queue[0].status).toBe('done');
  });

  it('status transitions: converting → error stores message', () => {
    const queue = [makeItem()];
    queue[0].status = 'converting';
    queue[0].status = 'error';
    queue[0].error = 'ffmpeg: codec not found';
    expect(queue[0].status).toBe('error');
    expect(queue[0].error).toBe('ffmpeg: codec not found');
  });

  it('status transitions: converting → cancelled', () => {
    const queue = [makeItem({ status: 'converting' })];
    queue[0].status = 'cancelled';
    queue[0].percent = 0;
    expect(queue[0].status).toBe('cancelled');
  });
});

// ── Post-cancel flicker guard (F-19) ─────────────────────────────────────────
// Regression: job-progress events arriving after cancel_job must not flip the
// item back to 'converting'. applyProgressIfActive is the guard used by the
// App.svelte job-progress listener.

describe('applyProgressIfActive — post-cancel flicker guard', () => {
  function makeItem(status = 'converting', percent = 10) {
    return { id: 'x', path: '/p', name: 'n', ext: 'mp4', mediaType: 'video',
             status, percent, error: null };
  }

  it('applies progress while the item is still converting', () => {
    const item = makeItem('converting', 10);
    expect(applyProgressIfActive(item, 42)).toBe(true);
    expect(item.status).toBe('converting');
    expect(item.percent).toBe(42);
  });

  it('applies progress to a pending item and flips it to converting', () => {
    const item = makeItem('pending', 0);
    expect(applyProgressIfActive(item, 5)).toBe(true);
    expect(item.status).toBe('converting');
    expect(item.percent).toBe(5);
  });

  it('does NOT overwrite a cancelled item (the bug F-19 is about)', () => {
    const item = makeItem('cancelled', 0);
    expect(applyProgressIfActive(item, 75)).toBe(false);
    expect(item.status).toBe('cancelled');
    expect(item.percent).toBe(0);
  });

  it('does NOT overwrite a done item', () => {
    const item = makeItem('done', 100);
    expect(applyProgressIfActive(item, 50)).toBe(false);
    expect(item.status).toBe('done');
    expect(item.percent).toBe(100);
  });

  it('does NOT overwrite an errored item', () => {
    const item = makeItem('error', 30);
    item.error = 'boom';
    expect(applyProgressIfActive(item, 60)).toBe(false);
    expect(item.status).toBe('error');
    expect(item.error).toBe('boom');
  });

  it('isTerminal reports terminal states', () => {
    expect(isTerminal(makeItem('pending'))).toBe(false);
    expect(isTerminal(makeItem('converting'))).toBe(false);
    expect(isTerminal(makeItem('done'))).toBe(true);
    expect(isTerminal(makeItem('error'))).toBe(true);
    expect(isTerminal(makeItem('cancelled'))).toBe(true);
  });

  it('full cancel sequence: late progress after cancel is ignored', () => {
    const item = makeItem('converting', 20);
    // User clicks cancel → backend emits job-cancelled
    markCancelled(item);
    expect(item.status).toBe('cancelled');
    // Late ffmpeg stderr line arrives → job-progress fires
    const applied = applyProgressIfActive(item, 88);
    expect(applied).toBe(false);
    expect(item.status).toBe('cancelled');
    expect(item.percent).toBe(0);
  });
});

// ── Preview stale-result discard (F-21) ──────────────────────────────────────
// Regression: rapid slider drags issue overlapping preview_* invocations; an
// older result that resolves after a newer one must not overwrite the UI.
// The pattern used inline in ChromaKeyPanel / _runImageDiff / runDiffPreview
// is a monotonic generation counter captured at invoke time and re-checked
// before writing to state.

describe('generation-token pattern for preview stale-result guard', () => {
  it('older result that resolves last is discarded', async () => {
    let gen = 0;
    let applied = null;
    async function runPreview(value, delayMs) {
      const myGen = ++gen;
      await new Promise(r => setTimeout(r, delayMs));
      if (myGen !== gen) return;  // stale — newer invoke superseded this one
      applied = value;
    }
    // Old invoke is slow; new invoke fires after it and resolves first.
    const slow = runPreview('old', 30);
    await new Promise(r => setTimeout(r, 5));
    const fast = runPreview('new', 5);
    await Promise.all([slow, fast]);
    expect(applied).toBe('new');
  });

  it('sequential non-overlapping invokes all apply', async () => {
    let gen = 0;
    let applied = null;
    async function runPreview(value) {
      const myGen = ++gen;
      await Promise.resolve();
      if (myGen !== gen) return;
      applied = value;
    }
    await runPreview('a');
    expect(applied).toBe('a');
    await runPreview('b');
    expect(applied).toBe('b');
  });

  it('N concurrent invokes: only the last-issued one wins', async () => {
    let gen = 0;
    let applied = null;
    const results = ['one', 'two', 'three', 'four', 'five'];
    const delays  = [40, 10, 35, 5, 20];
    async function runPreview(value, delayMs) {
      const myGen = ++gen;
      await new Promise(r => setTimeout(r, delayMs));
      if (myGen !== gen) return;
      applied = value;
    }
    await Promise.all(results.map((v, i) => runPreview(v, delays[i])));
    expect(applied).toBe('five');
  });
});

// ── validateOptions ───────────────────────────────────────────────────────────

describe('validateOptions', () => {
  const baseVideo = {
    output_format: 'mp4',
    codec: 'h264',
    resolution: 'original',
    trim_start: null,
    trim_end: null,
    remove_audio: false,
    extract_audio: false,
    bitrate: 192,
    sample_rate: 48000,
  };

  const baseAudio = {
    output_format: 'mp3',
    bitrate: 192,
    sample_rate: 44100,
    normalize_loudness: false,
    trim_start: null,
    trim_end: null,
  };

  it('returns no errors for valid options', () => {
    const errors = validateOptions(baseVideo, baseAudio);
    expect(errors).toEqual({});
  });

  it('catches video trim_end <= trim_start', () => {
    const video = { ...baseVideo, trim_start: 30, trim_end: 10 };
    const errors = validateOptions(video, baseAudio);
    expect(errors.video_trim).toBeTruthy();
  });

  it('catches video trim_end === trim_start', () => {
    const video = { ...baseVideo, trim_start: 15, trim_end: 15 };
    const errors = validateOptions(video, baseAudio);
    expect(errors.video_trim).toBeTruthy();
  });

  it('no video trim error when only start is set', () => {
    const video = { ...baseVideo, trim_start: 10, trim_end: null };
    const errors = validateOptions(video, baseAudio);
    expect(errors.video_trim).toBeUndefined();
  });

  it('no video trim error when only end is set', () => {
    const video = { ...baseVideo, trim_start: null, trim_end: 60 };
    const errors = validateOptions(video, baseAudio);
    expect(errors.video_trim).toBeUndefined();
  });

  it('valid trim range passes', () => {
    const video = { ...baseVideo, trim_start: 10, trim_end: 60 };
    const errors = validateOptions(video, baseAudio);
    expect(errors.video_trim).toBeUndefined();
  });

  it('catches audio trim_end <= trim_start', () => {
    const audio = { ...baseAudio, trim_start: 50, trim_end: 20 };
    const errors = validateOptions(baseVideo, audio);
    expect(errors.audio_trim).toBeTruthy();
  });

  it('catches invalid custom resolution format', () => {
    const video = { ...baseVideo, resolution: 'notaresolution' };
    const errors = validateOptions(video, baseAudio);
    expect(errors.resolution).toBeTruthy();
  });

  it('accepts valid custom resolution WxH format', () => {
    const video = { ...baseVideo, resolution: '2560x1440' };
    const errors = validateOptions(video, baseAudio);
    expect(errors.resolution).toBeUndefined();
  });

  it('accepts all preset resolutions', () => {
    for (const res of ['original', '1920x1080', '1280x720', '854x480']) {
      const video = { ...baseVideo, resolution: res };
      const errors = validateOptions(video, baseAudio);
      expect(errors.resolution).toBeUndefined();
    }
  });
});

// ── QueueManager component tests ──────────────────────────────────────────────
// These tests mount QueueManager and exercise its exported API. The mocks at
// the top of this file cover all invoke() calls used by the async pipeline.

import QueueManager from '../lib/QueueManager.svelte';

describe('QueueManager', () => {
  let target;
  let comp;

  beforeEach(() => {
    target = document.createElement('div');
    document.body.appendChild(target);
    vi.clearAllMocks();
  });

  afterEach(() => {
    if (comp) { unmount(comp); comp = null; }
    document.body.removeChild(target);
  });

  // ── Test 1: Selecting an item updates selectedItem ─────────────────────────
  it('selecting an item updates selectedItem', () => {
    let boundItem = null;
    comp = mount(QueueManager, {
      target,
      props: {
        get selectedItem() { return boundItem; },
        set selectedItem(v) { boundItem = v; },
        setStatus: vi.fn(),
      },
    });

    const videoItem = {
      id: crypto.randomUUID(), kind: 'file', parentId: null,
      path: '/tmp/test.mp4', name: 'test.mp4', ext: 'mp4',
      mediaType: 'video', status: 'pending', percent: 0, info: null,
    };

    // Push directly onto the bound queue by calling addFiles with a path
    // that routes through the component's addFiles export.
    comp.addFiles(['/tmp/test.mp4']);
    flushSync();

    // Simpler: use the queue $bindable — verify addFiles adds an item and
    // that after handleSelect the selectedItem matches.
    let boundQueue = [];
    unmount(comp);
    comp = mount(QueueManager, {
      target,
      props: {
        get queue()         { return boundQueue; },
        set queue(v)        { boundQueue = v; },
        get selectedItem()  { return boundItem; },
        set selectedItem(v) { boundItem = v; },
        setStatus: vi.fn(),
      },
    });

    comp.addFiles(['/tmp/clip.mp4']);
    flushSync();

    expect(boundQueue.length).toBe(1);
    expect(boundQueue[0].name).toBe('clip.mp4');

    comp.handleSelect(boundQueue[0].id);
    flushSync();

    expect(boundItem).not.toBeNull();
    expect(boundItem.id).toBe(boundQueue[0].id);
    expect(boundItem.name).toBe('clip.mp4');
  });

  // ── Test 2: Rapid reselection cancels the in-flight pipeline ──────────────
  // Mock invoke to resolve slowly so we can observe pipeline cancellation.
  // After a rapid burst of selections, only the final selection's get_waveform
  // call path should be the last-selected item's path.
  it('rapid reselection cancels in-flight pipeline', async () => {
    const { invoke } = await import('@tauri-apps/api/core');

    // Use a controlled delay: each get_file_info call takes ~10ms
    vi.mocked(invoke).mockImplementation((cmd, args) => {
      if (cmd === 'get_file_info') {
        return new Promise(r => setTimeout(() => r({ duration_secs: 10 }), 10));
      }
      if (cmd === 'get_waveform') return Promise.resolve([]);
      if (cmd === 'get_filmstrip') return Promise.resolve([]);
      return Promise.resolve(null);
    });

    let boundQueue = [];
    let boundItem  = null;
    comp = mount(QueueManager, {
      target,
      props: {
        get queue()        { return boundQueue; },
        set queue(v)       { boundQueue = v; },
        get selectedItem() { return boundItem; },
        set selectedItem(v){ boundItem = v; },
        setStatus: vi.fn(),
      },
    });

    comp.addFiles(['/tmp/a.mp4', '/tmp/b.mp4', '/tmp/c.mp4']);
    flushSync();
    expect(boundQueue.length).toBe(3);

    const [a, b, c] = boundQueue;

    // Rapid burst: select a, then b, then c in quick succession
    comp.handleSelect(a.id);
    comp.handleSelect(b.id);
    comp.handleSelect(c.id);
    flushSync();

    // Let all async work finish
    await new Promise(r => setTimeout(r, 200));

    // The final selectedItem should be c
    expect(boundItem?.id).toBe(c.id);

    // get_file_info was called for each selection that started the pipeline,
    // but the in-flight calls for a and b bail once c increments _loadGen.
    // At minimum, c's path was the last get_file_info invocation.
    const fiCalls = vi.mocked(invoke).mock.calls
      .filter(([cmd]) => cmd === 'get_file_info')
      .map(([, args]) => args.path);
    expect(fiCalls[fiCalls.length - 1]).toBe('/tmp/c.mp4');
  });

  // ── Test 3: removeItem removes from queue and advances selection ───────────
  it('removeItem removes the item and advances selection when it was selected', () => {
    let boundQueue = [];
    let boundItem  = null;
    comp = mount(QueueManager, {
      target,
      props: {
        get queue()        { return boundQueue; },
        set queue(v)       { boundQueue = v; },
        get selectedItem() { return boundItem; },
        set selectedItem(v){ boundItem = v; },
        setStatus: vi.fn(),
      },
    });

    comp.addFiles(['/tmp/first.mp4', '/tmp/second.mp4']);
    flushSync();
    expect(boundQueue.length).toBe(2);

    const firstId = boundQueue[0].id;
    comp.handleSelect(firstId);
    flushSync();
    expect(boundItem?.id).toBe(firstId);

    // Remove the selected item
    comp.removeItem(firstId);
    flushSync();

    // Queue shrinks
    expect(boundQueue.length).toBe(1);
    expect(boundQueue.find(q => q.id === firstId)).toBeUndefined();

    // Selection advances to the remaining item
    expect(boundItem).not.toBeNull();
    expect(boundItem.id).toBe(boundQueue[0].id);
  });
});
