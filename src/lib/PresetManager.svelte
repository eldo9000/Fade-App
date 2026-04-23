<script>
  import { invoke } from '@tauri-apps/api/core';

  let {
    imageOptions = $bindable(),
    videoOptions = $bindable(),
    audioOptions = $bindable(),
    globalOutputFormat = $bindable(),
    activeOutputCategory,
    presetsMode   = $bindable(false),
    combinedPresets = $bindable([]),
  } = $props();

  const BUILTIN_PRESETS = {
    audio: [
      { id: '__b_streaming', name: 'Streaming',    media_type: 'audio', output_format: 'mp3',  bitrate: 192,  sample_rate: 44100, normalize_loudness: false },
      { id: '__b_voice',     name: 'Voice only',   media_type: 'audio', output_format: 'mp3',  bitrate: 64,   sample_rate: 44100, normalize_loudness: true  },
      { id: '__b_cd',        name: 'CD quality',   media_type: 'audio', output_format: 'mp3',  bitrate: 320,  sample_rate: 44100, normalize_loudness: false },
      { id: '__b_lossless',  name: 'Lossless',     media_type: 'audio', output_format: 'flac', bitrate: null, sample_rate: 44100, normalize_loudness: false },
      { id: '__b_podcast',   name: 'Podcast',      media_type: 'audio', output_format: 'mp3',  bitrate: 128,  sample_rate: 44100, normalize_loudness: true  },
      { id: '__b_opus',      name: 'Opus (small)', media_type: 'audio', output_format: 'opus', bitrate: 96,   sample_rate: 48000, normalize_loudness: false },
    ],
    video: [],
    image: [],
  };
  const ALL_BUILTINS = Object.values(BUILTIN_PRESETS).flat();

  let presets      = $state([]);
  let headerAdding = $state(false);
  let headerPresetName = $state('');

  $effect(() => { combinedPresets = [...ALL_BUILTINS, ...presets]; });

  export async function loadPresets() {
    try { presets = await invoke('list_presets'); } catch { /* no-op */ }
  }

  export function applyPreset(id) {
    const p = ALL_BUILTINS.find(b => b.id === id) ?? presets.find(p => p.id === id);
    if (!p) return;
    if (p.media_type === 'image') {
      imageOptions.output_format = p.output_format;
      if (p.quality != null) imageOptions.quality = p.quality;
    } else if (p.media_type === 'video') {
      videoOptions.output_format = p.output_format;
      if (p.codec != null)        videoOptions.codec = p.codec;
      if (p.bitrate != null)      videoOptions.bitrate = p.bitrate;
      if (p.sample_rate != null)  videoOptions.sample_rate = p.sample_rate;
    } else {
      audioOptions.output_format = p.output_format;
      if (p.bitrate != null)           audioOptions.bitrate = p.bitrate;
      if (p.sample_rate != null)       audioOptions.sample_rate = p.sample_rate;
      if (p.normalize_loudness != null) audioOptions.normalize_loudness = p.normalize_loudness;
    }
    globalOutputFormat = p.output_format;
  }

  async function saveHeaderPreset() {
    const name = headerPresetName.trim();
    if (!name || !activeOutputCategory) return;
    const tab = activeOutputCategory;
    const src = tab === 'image' ? imageOptions : tab === 'video' ? videoOptions : audioOptions;
    try {
      const saved = await invoke('save_preset', {
        name, mediaType: tab,
        outputFormat: src.output_format,
        quality: tab === 'image' ? (src.quality ?? null) : null,
        codec: tab === 'video' ? (src.codec ?? null) : null,
        bitrate: (tab === 'video' || tab === 'audio') ? (src.bitrate ?? null) : null,
        sampleRate: (tab === 'video' || tab === 'audio') ? (src.sample_rate ?? null) : null,
        normalizeLoudness: tab === 'audio' ? (src.normalize_loudness ?? null) : null,
      });
      presets = [...presets, saved];
      headerPresetName = '';
      headerAdding = false;
    } catch (e) { console.error('Save preset failed:', e); }
  }

  export async function deletePreset(id) {
    try {
      await invoke('delete_preset', { id });
      presets = presets.filter(p => p.id !== id);
    } catch (e) { console.error('Delete preset failed:', e); }
  }
</script>

{#if headerAdding}
  <div class="flex gap-1 ml-auto">
    <!-- svelte-ignore a11y_autofocus -->
    <input
      type="text"
      bind:value={headerPresetName}
      placeholder="Name…"
      data-tooltip="Name the preset — Enter saves, Esc cancels."
      autofocus
      onkeydown={(e) => { if (e.key === 'Enter') saveHeaderPreset(); if (e.key === 'Escape') { headerAdding = false; headerPresetName = ''; } }}
      class="w-24 px-3 py-1 rounded-l text-[13px] border border-[var(--border)]
             bg-[var(--surface)] text-[var(--text-primary)] outline-none
             focus:border-[var(--accent)] transition-colors"
    />
    <button onclick={saveHeaderPreset}
            class="px-3 py-1 text-[13px] font-semibold border -ml-px border-[var(--accent)]
                   bg-[var(--accent)] text-white rounded-r hover:opacity-90 transition-opacity">
      Save
    </button>
    <button onclick={() => { headerAdding = false; headerPresetName = ''; }}
            class="px-3 py-1 text-[13px] border border-[var(--border)] rounded
                   text-[var(--text-secondary)] hover:text-[var(--text-primary)] transition-colors">
      ✕
    </button>
  </div>
{:else}
  <div class="flex gap-1 ml-auto">
    <button
      onclick={() => presetsMode = !presetsMode}
      data-tooltip={presetsMode ? 'Close preset picker — return to output options.' : 'Browse and apply saved presets — takes over the panel below.'}
      class="px-3 py-1 text-[13px] font-semibold rounded bg-[var(--accent)] text-white
             hover:opacity-90 transition-opacity shrink-0"
    >
      {presetsMode ? '← Back' : 'Presets'}
    </button>
    <button
      onclick={() => { headerAdding = true; headerPresetName = ''; }}
      disabled={!activeOutputCategory || !['image','video','audio'].includes(activeOutputCategory)}
      data-tooltip="Save the current panel settings as a named preset — reusable across files and sessions."
      title="Save current settings as preset"
      class="w-7 py-1 text-[15px] border border-[var(--border)] rounded flex items-center justify-center
             text-[var(--text-secondary)] hover:bg-[var(--accent)] hover:text-white hover:border-[color-mix(in_srgb,var(--accent)_70%,#000)]
             transition-colors leading-none disabled:opacity-30 disabled:cursor-not-allowed"
    >+</button>
    <button
      disabled
      data-tooltip="Select a preset in the panel to delete it."
      title="Delete preset"
      class="w-7 py-1 text-[15px] border border-[var(--border)] rounded flex items-center justify-center
             text-[var(--text-secondary)] transition-colors leading-none opacity-30 cursor-not-allowed"
    >−</button>
  </div>
{/if}
