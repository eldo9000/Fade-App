export const ZOOM_STEPS = [1.0, 1.1, 1.2, 1.3, 1.4, 1.5, 1.6, 1.7, 1.8];
const KEY = 'zoomLevel';

function loadInitial() {
  const raw = parseFloat(localStorage.getItem(KEY));
  return ZOOM_STEPS.includes(raw) ? raw : 1.0;
}

export function createZoom() {
  let level = $state(loadInitial());

  function apply(next) {
    level = next;
    localStorage.setItem(KEY, String(next));
    document.documentElement.style.zoom = String(next);
  }

  function stepIn()  { const i = ZOOM_STEPS.indexOf(level); if (i < ZOOM_STEPS.length - 1) apply(ZOOM_STEPS[i + 1]); }
  function stepOut() { const i = ZOOM_STEPS.indexOf(level); if (i > 0) apply(ZOOM_STEPS[i - 1]); }
  function reset()   { apply(1.0); }

  function handleKey(e) {
    if (!e.metaKey) return;
    if (e.key !== '=' && e.key !== '+' && e.key !== '-' && e.key !== '0') return;
    e.preventDefault();
    if (e.key === '=' || e.key === '+') stepIn();
    else if (e.key === '-') stepOut();
    else if (e.key === '0') reset();
  }

  return {
    get level() { return level; },
    apply, stepIn, stepOut, reset, handleKey,
  };
}
