// Shared dropdown overlay state — rendered at the App.svelte root so it
// escapes every overflow/stacking context inside the panel hierarchy.
export const overlay = $state({
  open: false,
  anchorEl: null,
  anchorRect: null,
  items: [],
  onPick: null,
});

export function showOverlay(el, rect, its, fn) {
  overlay.anchorEl = el;
  overlay.anchorRect = { left: rect.left, top: rect.top, right: rect.right, bottom: rect.bottom, width: rect.width, height: rect.height };
  overlay.items = its;
  overlay.onPick = fn;
  overlay.open = true;
}

export function hideOverlay() {
  overlay.open = false;
}
