<script>
  import { invoke } from '@tauri-apps/api/core';

  let {
    imageOptions = $bindable(),
    videoOptions = $bindable(),
    audioOptions = $bindable(),
    globalOutputFormat = $bindable(),
    activeOutputCategory,
    setStatus,
  } = $props();

  // Built-in presets — always available, never persisted to backend
  const BUILTIN_PRESETS = {
    audio: [
      { id: '__b_streaming',   name: 'Streaming',    media_type: 'audio', output_format: 'mp3',  bitrate: 192,  sample_rate: 44100, normalize_loudness: false },
      { id: '__b_voice',       name: 'Voice only',   media_type: 'audio', output_format: 'mp3',  bitrate: 64,   sample_rate: 44100, normalize_loudness: true  },
      { id: '__b_cd',          name: 'CD quality',   media_type: 'audio', output_format: 'mp3',  bitrate: 320,  sample_rate: 44100, normalize_loudness: false },
      { id: '__b_lossless',    name: 'Lossless',     media_type: 'audio', output_format: 'flac', bitrate: null, sample_rate: 44100, normalize_loudness: false },
      { id: '__b_podcast',     name: 'Podcast',      media_type: 'audio', output_format: 'mp3',  bitrate: 128,  sample_rate: 44100, normalize_loudness: true  },
      { id: '__b_opus',        name: 'Opus (small)', media_type: 'audio', output_format: 'opus', bitrate: 96,   sample_rate: 48000, normalize_loudness: false },
    ],
    video: [],
    image: [],
  };
  const ALL_BUILTINS = Object.values(BUILTIN_PRESETS).flat();

  let presets          = $state([]);
  let headerPresetId   = $state('');
  let headerAdding     = $state(false);
  let headerPresetName = $state('');
  let _hpSuppressReset = false; // plain bool, prevents auto-reset during apply

  // Auto-reset to "Presets" placeholder when the active settings change
  $effect(() => {
    void [
      imageOptions.output_format, imageOptions.quality,
      videoOptions.output_format, videoOptions.codec, videoOptions.bitrate, videoOptions.sample_rate,
      audioOptions.output_format, audioOptions.bitrate, audioOptions.sample_rate,
      activeOutputCategory,
    ];
    if (!_hpSuppressReset) headerPresetId = '';
  });

  export async function loadPresets() {
    try { presets = await invoke('list_presets'); } catch { /* no-op */ }
  }

  function applyPreset(id) {
    _hpSuppressReset = true;
    const p = ALL_BUILTINS.find(b => b.id === id) ?? presets.find(p => p.id === id);
    if (!p) return;
    if (p.media_type === 'image') {
      imageOptions.output_format = p.output_format;
      if (p.quality != null) imageOptions.quality = p.quality;
    } else if (p.media_type === 'video') {
      videoOptions.output_format = p.output_format;
      if (p.codec != null) videoOptions.codec = p.codec;
      if (p.bitrate != null) videoOptions.bitrate = p.bitrate;
      if (p.sample_rate != null) videoOptions.sample_rate = p.sample_rate;
    } else {
      audioOptions.output_format = p.output_format;
      if (p.bitrate != null) audioOptions.bitrate = p.bitrate;
      if (p.sample_rate != null) audioOptions.sample_rate = p.sample_rate;
      if (p.normalize_loudness != null) audioOptions.normalize_loudness = p.normalize_loudness;
    }
    // Also sync globalOutputFormat so the header button reflects the preset's format
    globalOutputFormat = p.output_format;
    queueMicrotask(() => { _hpSuppressReset = false; });
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
      });
      presets = [...presets, saved];
      headerPresetName = '';
      headerAdding = false;
      _hpSuppressReset = true;
      headerPresetId = saved.id;
      queueMicrotask(() => { _hpSuppressReset = false; });
    } catch (e) { console.error('Save preset failed:', e); }
  }

  async function deletePreset(id) {
    try {
      await invoke('delete_preset', { id });
      presets = presets.filter(p => p.id !== id);
      if (headerPresetId === id) headerPresetId = '';
    } catch (e) { console.error('Delete preset failed:', e); }
  }
</script>

<!-- Presets selector — always visible; filtered to active category when one is selected -->
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
    <select
      bind:value={headerPresetId}
      data-tooltip="Preset picker — one-click load a saved or built-in bundle of settings. Filtered to the current output category."
      onchange={(e) => { const id = e.currentTarget.value; if (id) applyPreset(id); }}
      class="px-3 py-1 rounded text-[13px] font-semibold border border-[var(--border)]
             bg-[var(--surface)] outline-none transition-colors cursor-pointer
             hover:border-[var(--accent)]
             {headerPresetId ? 'text-[var(--text-primary)]' : 'text-[var(--text-secondary)]'}"
    >
      <option value="">Presets</option>
      {#if activeOutputCategory}
        {#each (BUILTIN_PRESETS[activeOutputCategory] ?? []) as p (p.id)}
          <option value={p.id}>{p.name}</option>
        {/each}
        {#each presets.filter(p => p.media_type === activeOutputCategory) as p (p.id)}
          <option value={p.id}>{p.name}</option>
        {/each}
      {:else}
        {#each ALL_BUILTINS as p (p.id)}
          <option value={p.id}>{p.name}</option>
        {/each}
        {#each presets as p (p.id)}
          <option value={p.id}>{p.name}</option>
        {/each}
      {/if}
    </select>
    <button
      onclick={() => { headerAdding = true; headerPresetName = ''; }}
      disabled={!activeOutputCategory || !['image','video','audio'].includes(activeOutputCategory)}
      data-tooltip="Save the current panel settings as a named preset — reusable across files and sessions."
      title="Save current settings as preset"
      class="w-9 py-1 text-[15px] border border-[var(--border)] rounded flex items-center justify-center
             text-[var(--text-secondary)] hover:text-[var(--accent)] hover:border-[var(--accent)]
             transition-colors leading-none disabled:opacity-30 disabled:cursor-not-allowed"
    >+</button>
    <button
      onclick={() => deletePreset(headerPresetId)}
      disabled={!headerPresetId || headerPresetId.startsWith('__b_')}
      data-tooltip="Delete the selected preset — only works on your saved presets, not built-ins."
      title="Delete this preset"
      class="w-9 py-1 text-[15px] border border-[var(--border)] rounded flex items-center justify-center
             text-[var(--text-secondary)] hover:text-red-400 hover:border-red-500
             transition-colors leading-none disabled:opacity-30 disabled:cursor-not-allowed"
    >−</button>
  </div>
{/if}
