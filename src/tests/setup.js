// Browser API polyfills for jsdom
//
// jsdom with an opaque origin (default about:blank) exposes a `localStorage`
// property, but accessing its methods throws SecurityError / TypeError depending
// on version. Overwrite with a plain Map-backed shim before any module under
// test reads from it.
{
  const store = new Map();
  const shim = {
    getItem: (k) => (store.has(k) ? store.get(k) : null),
    setItem: (k, v) => { store.set(String(k), String(v)); },
    removeItem: (k) => { store.delete(k); },
    clear: () => { store.clear(); },
    key: (i) => Array.from(store.keys())[i] ?? null,
    get length() { return store.size; },
  };
  try { Object.defineProperty(globalThis, 'localStorage', { value: shim, configurable: true }); } catch {}
  try { Object.defineProperty(globalThis, 'sessionStorage', { value: shim, configurable: true }); } catch {}
  if (typeof window !== 'undefined') {
    try { Object.defineProperty(window, 'localStorage',  { value: shim, configurable: true }); } catch {}
    try { Object.defineProperty(window, 'sessionStorage',{ value: shim, configurable: true }); } catch {}
  }
}

if (typeof ResizeObserver === 'undefined') {
  global.ResizeObserver = class ResizeObserver {
    observe() {}
    unobserve() {}
    disconnect() {}
  };
}
if (typeof IntersectionObserver === 'undefined') {
  global.IntersectionObserver = class IntersectionObserver {
    observe() {}
    unobserve() {}
    disconnect() {}
  };
}
