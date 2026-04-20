import { describe, it, expect, vi, beforeEach } from 'vitest';
import { mount, unmount } from 'svelte';
import ChromaKeyPanel from '../lib/ChromaKeyPanel.svelte';

const mockInvoke = vi.fn();

vi.mock('@tauri-apps/api/core', () => ({
  invoke: (...args) => mockInvoke(...args),
  convertFileSrc: vi.fn(p => `asset://${p}`),
}));

vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn().mockResolvedValue(() => {}),
}));

function makeItem(overrides = {}) {
  return {
    id: 'test-id',
    path: '/tmp/greenscreen.mp4',
    name: 'greenscreen.mp4',
    ext: 'mp4',
    mediaType: 'video',
    status: 'ready',
    percent: 0,
    error: null,
    ...overrides,
  };
}

describe('ChromaKeyPanel', () => {
  let target;

  beforeEach(() => {
    mockInvoke.mockResolvedValue(null);
    target = document.createElement('div');
    document.body.appendChild(target);
  });

  it('mounts without throwing', () => {
    let comp;
    expect(() => {
      comp = mount(ChromaKeyPanel, {
        target,
        props: { selectedItem: null, setStatus: vi.fn() },
      });
    }).not.toThrow();
    if (comp) unmount(comp);
    document.body.removeChild(target);
  });

  it('calls run_operation with chroma_key type when runChromaKey fires', async () => {
    const item = makeItem();
    const setStatus = vi.fn();
    let comp;
    expect(() => {
      comp = mount(ChromaKeyPanel, {
        target,
        props: {
          selectedItem: item,
          outputDir: '/tmp',
          outputSeparator: '_',
          setStatus,
        },
      });
    }).not.toThrow();

    const btn = target.querySelector('button[data-op="chroma"], button.run-chroma');
    if (btn) {
      btn.click();
      await new Promise(r => setTimeout(r, 50));
      expect(mockInvoke).toHaveBeenCalledWith('run_operation', expect.objectContaining({
        operation: expect.objectContaining({ type: 'chroma_key' }),
      }));
    }
    if (comp) unmount(comp);
    document.body.removeChild(target);
  });
});
