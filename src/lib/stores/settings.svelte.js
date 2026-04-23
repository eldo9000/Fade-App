const KEY = 'fade-settings';

const DEFAULTS = {
  notifyUpdates: true,
  notifyFrequency: 'weekly',     // hard-coded for now; UI hidden
  autoUpdate: false,
  vizDefault: 'no',
  limiterAuto: false,
  autoCompact: true,
  fileTypeColumn: true,
  brightFiletype: false,
  lastUpdateCheck: 0,            // epoch ms; 0 = never. Gates the weekly check.
  sendDiagnostics: true,         // opt-in after 1.0; forced-on during beta (see BETA flag).
  showDevFeatures: true,         // show preview/todo items (green) in the format picker; disable for a clean production view
  conversionCollapsed: false,    // right-sidebar "Conversion" super-section fold state
  toolsCollapsed: false,         // right-sidebar "Tools" super-section fold state
  filesCollapsed: true,          // right-sidebar "Files" super-section fold state (data / doc / archive)
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
