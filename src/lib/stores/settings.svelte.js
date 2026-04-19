const KEY = 'fade-settings';

const DEFAULTS = {
  notifyUpdates: true,
  notifyFrequency: 'weekly',     // hard-coded for now; UI hidden
  autoUpdate: false,
  vizDefault: 'no',
  limiterAuto: false,
  previewHighQuality: false,
  autoCompact: true,
  hideConverted: true,
  fileTypeColumn: true,
};

function load() {
  try {
    return { ...DEFAULTS, ...JSON.parse(localStorage.getItem(KEY) ?? '{}') };
  } catch {
    return { ...DEFAULTS };
  }
}

/**
 * Returns the settings state object directly. Mutations persist to localStorage
 * via an $effect set up at the call site (must be inside a component).
 * Consumers use dot-notation reads/writes: `settings.autoCompact = false`.
 */
export function createSettings() {
  const state = $state(load());
  $effect(() => { localStorage.setItem(KEY, JSON.stringify(state)); });
  return state;
}
