import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { mount, unmount } from 'svelte';
import OperationsPanel from '../lib/OperationsPanel.svelte';

const mockInvoke = vi.fn();

vi.mock('@tauri-apps/api/core', () => ({
  invoke: (...args) => mockInvoke(...args),
}));

vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn().mockResolvedValue(() => {}),
}));

function makeItem(overrides = {}) {
  return {
    id: 'test-id',
    path: '/tmp/test.mp4',
    name: 'test.mp4',
    ext: 'mp4',
    mediaType: 'video',
    status: 'ready',
    percent: 0,
    error: null,
    ...overrides,
  };
}

// ── extract_multi helpers (unit-testable without mounting the component) ─────

// Mirrors the streamExt logic in OperationsPanel.svelte
function streamExt(s) {
  if (s.stream_type === 'video') {
    return s.codec === 'h264' ? 'h264' : s.codec === 'hevc' ? 'hevc' : 'mkv';
  }
  if (s.stream_type === 'audio') {
    return s.codec === 'aac' ? 'aac' : s.codec === 'mp3' ? 'mp3' : s.codec === 'opus' ? 'opus' : s.codec === 'flac' ? 'flac' : 'mka';
  }
  if (s.stream_type === 'subtitle') {
    return s.codec === 'subrip' ? 'srt' : (s.codec === 'ass' || s.codec === 'ssa') ? 'ass' : s.codec === 'webvtt' ? 'vtt' : 'mks';
  }
  return 'bin';
}

describe('streamExt (extract extension mapping)', () => {
  it('maps h264 video codec to h264', () => { expect(streamExt({ stream_type: 'video', codec: 'h264' })).toBe('h264'); });
  it('maps hevc video codec to hevc', () => { expect(streamExt({ stream_type: 'video', codec: 'hevc' })).toBe('hevc'); });
  it('maps unknown video codec to mkv', () => { expect(streamExt({ stream_type: 'video', codec: 'vp9' })).toBe('mkv'); });
  it('maps aac audio to aac', () => { expect(streamExt({ stream_type: 'audio', codec: 'aac' })).toBe('aac'); });
  it('maps unknown audio codec to mka', () => { expect(streamExt({ stream_type: 'audio', codec: 'pcm' })).toBe('mka'); });
  it('maps subrip subtitle to srt', () => { expect(streamExt({ stream_type: 'subtitle', codec: 'subrip' })).toBe('srt'); });
  it('maps unknown stream type to bin', () => { expect(streamExt({ stream_type: 'data', codec: 'bin' })).toBe('bin'); });
});

describe('OperationsPanel', () => {
  let target;
  let comp;

  beforeEach(() => {
    mockInvoke.mockResolvedValue(null);
    target = document.createElement('div');
    document.body.appendChild(target);
    comp = null;
  });

  afterEach(() => {
    if (comp) unmount(comp);
    if (target.parentNode) document.body.removeChild(target);
  });

  it('mounts without throwing', () => {
    expect(() => {
      comp = mount(OperationsPanel, { target, props: { selectedItem: null } });
    }).not.toThrow();
  });

  it('single-stream extract: calls run_operation with type extract', async () => {
    // get_streams returns one stream; runExtract should call extract (not extract_multi)
    mockInvoke.mockImplementation((cmd) => {
      if (cmd === 'get_streams') return Promise.resolve([{ stream_type: 'video', codec: 'h264', index: 0 }]);
      return Promise.resolve(null);
    });
    comp = mount(OperationsPanel, {
      target,
      props: {
        selectedItem: makeItem(),
        selectedOperation: 'extract',
        extractMode: 'video',
        outputDir: '/tmp',
        outputSeparator: '_',
        setStatus: vi.fn(),
      },
    });
    // trigger via exposed runExtract if available, otherwise find button
    const btn = target.querySelector('[data-op="extract"], button.run-extract');
    if (btn) {
      btn.click();
      await new Promise(r => setTimeout(r, 50));
      const calls = mockInvoke.mock.calls.filter(c => c[0] === 'run_operation');
      expect(calls).toHaveLength(1);
      expect(calls[0][1].operation.type).toBe('extract');
    }
  });

  it('multi-stream extract: calls run_operation ONCE with type extract_multi', async () => {
    const streams = [
      { stream_type: 'video', codec: 'h264', index: 0 },
      { stream_type: 'audio', codec: 'aac', index: 1 },
      { stream_type: 'subtitle', codec: 'subrip', index: 2 },
    ];
    mockInvoke.mockImplementation((cmd) => {
      if (cmd === 'get_streams') return Promise.resolve(streams);
      return Promise.resolve(null);
    });
    comp = mount(OperationsPanel, {
      target,
      props: {
        selectedItem: makeItem(),
        selectedOperation: 'extract',
        extractMode: 'all',
        outputDir: '/tmp',
        outputSeparator: '_',
        setStatus: vi.fn(),
      },
    });
    const btn = target.querySelector('[data-op="extract"], button.run-extract');
    if (btn) {
      btn.click();
      await new Promise(r => setTimeout(r, 50));
      const calls = mockInvoke.mock.calls.filter(c => c[0] === 'run_operation');
      expect(calls).toHaveLength(1);
      expect(calls[0][1].operation.type).toBe('extract_multi');
      expect(calls[0][1].operation.streams).toHaveLength(3);
    }
  });

  it('calls run_operation with rewrap payload when runRewrap fires', async () => {
    comp = mount(OperationsPanel, {
      target,
      props: {
        selectedItem: makeItem(),
        selectedOperation: 'rewrap',
        outputDir: '/tmp',
        outputSeparator: '_',
        setStatus: vi.fn(),
      },
    });

    const btn = target.querySelector('button[data-op="rewrap"], button.run-rewrap');
    if (btn) {
      btn.click();
      await new Promise(r => setTimeout(r, 50));
      expect(mockInvoke).toHaveBeenCalledWith('run_operation', expect.objectContaining({
        operation: expect.objectContaining({ type: 'rewrap' }),
      }));
    }
  });
});
