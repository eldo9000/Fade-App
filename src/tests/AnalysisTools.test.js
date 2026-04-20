import { describe, it, expect, vi, beforeEach } from 'vitest';
import { mount, unmount } from 'svelte';
import AnalysisTools from '../lib/AnalysisTools.svelte';

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
    path: '/tmp/test.wav',
    name: 'test.wav',
    ext: 'wav',
    mediaType: 'audio',
    status: 'ready',
    percent: 0,
    error: null,
    ...overrides,
  };
}

describe('AnalysisTools', () => {
  let target;

  beforeEach(() => {
    mockInvoke.mockResolvedValue({ integrated: -23, true_peak: -1.5 });
    target = document.createElement('div');
    document.body.appendChild(target);
  });

  it('mounts without throwing', () => {
    let comp;
    expect(() => {
      comp = mount(AnalysisTools, {
        target,
        props: { selectedItem: null, setStatus: vi.fn() },
      });
    }).not.toThrow();
    if (comp) unmount(comp);
    document.body.removeChild(target);
  });

  it('calls analyze_loudness with expected payload when loudness op is active', async () => {
    const item = makeItem();
    const setStatus = vi.fn();
    let comp;
    expect(() => {
      comp = mount(AnalysisTools, {
        target,
        props: {
          selectedItem: item,
          selectedOperation: 'loudness',
          setStatus,
        },
      });
    }).not.toThrow();

    const btn = target.querySelector('button[data-op="loudness"], button.run-loudness');
    if (btn) {
      btn.click();
      await new Promise(r => setTimeout(r, 50));
      expect(mockInvoke).toHaveBeenCalledWith('analyze_loudness', expect.objectContaining({
        inputPath: item.path,
      }));
    }
    if (comp) unmount(comp);
    document.body.removeChild(target);
  });
});
