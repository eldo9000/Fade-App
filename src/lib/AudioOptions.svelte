<script>
  let { options = $bindable(), errors = {} } = $props();

  const formats = ['mp3','flac','wav','ogg','aac','opus','m4a'];
  const bitrates = [64, 128, 192, 256, 320];
  const sampleRates = [
    { value: 44100, label: '44.1 kHz (CD)' },
    { value: 48000, label: '48 kHz (video standard)' },
    { value: 96000, label: '96 kHz (Hi-Fi)' },
    { value: 192000, label: '192 kHz (Archival)' },
  ];
  const presets = [
    { label: 'Streaming',    fmt: 'mp3',  br: 192, sr: 44100, norm: false },
    { label: 'Voice only',   fmt: 'mp3',  br: 64,  sr: 44100, norm: true  },
    { label: 'CD quality',   fmt: 'mp3',  br: 320, sr: 44100, norm: false },
    { label: 'Lossless',     fmt: 'flac', br: null, sr: 44100, norm: false },
    { label: 'Podcast',      fmt: 'mp3',  br: 128, sr: 44100, norm: true  },
    { label: 'Opus (small)', fmt: 'opus', br: 96,  sr: 48000, norm: false },
  ];

  function applyPreset(p) {
    options.output_format = p.fmt;
    if (p.br != null) options.bitrate = p.br;
    options.sample_rate = p.sr;
    options.normalize_loudness = p.norm;
  }

  const isLossless = $derived(['flac','wav','aiff','alac'].includes(options.output_format));

  function parseTime(raw) {
    if (!raw && raw !== 0) return null;
    const s = String(raw).trim();
    if (!s) return null;
    if (s.includes(':')) {
      const parts = s.split(':');
      return parseInt(parts[0], 10) * 60 + parseFloat(parts[1]);
    }
    return parseFloat(s) || null;
  }

  function formatTime(secs) {
    if (secs == null) return '';
    const m = Math.floor(secs / 60);
    const s = (secs % 60).toFixed(1);
    return `${m}:${s.padStart(4, '0')}`;
  }

  let trimStartRaw = $state(options.trim_start != null ? formatTime(options.trim_start) : '');
  let trimEndRaw   = $state(options.trim_end   != null ? formatTime(options.trim_end)   : '');

  function onTrimStartChange() { options.trim_start = parseTime(trimStartRaw); }
  function onTrimEndChange()   { options.trim_end   = parseTime(trimEndRaw);   }
  function clearTrim() {
    trimStartRaw = ''; trimEndRaw = '';
    options.trim_start = null; options.trim_end = null;
  }
</script>

<div class="space-y-5" role="form" aria-label="Audio conversion options">

  <!-- Output format -->
  <fieldset>
    <legend class="text-[12px] font-medium text-[var(--text-secondary)] uppercase tracking-wide mb-2">
      Output Format
    </legend>
    <div class="flex flex-wrap gap-2">
      {#each formats as fmt}
        <button
          onclick={() => options.output_format = fmt}
          class="px-3 py-1 rounded text-[12px] font-medium border transition-colors
            {options.output_format === fmt
              ? 'bg-[var(--accent)] text-white border-[var(--accent)]'
              : 'border-[var(--border)] text-[var(--text-primary)] hover:border-[var(--accent)]'}"
        >{fmt.toUpperCase()}</button>
      {/each}
    </div>
    {#if isLossless}
      <p class="text-[11px] text-[var(--text-secondary)] mt-1.5">Lossless — bitrate setting not applicable.</p>
    {/if}
  </fieldset>

  <!-- Quick presets -->
  <fieldset>
    <legend class="text-[12px] font-medium text-[var(--text-secondary)] uppercase tracking-wide mb-2">
      Quick Presets
    </legend>
    <div class="flex flex-wrap gap-2">
      {#each presets as p}
        <button
          onclick={() => applyPreset(p)}
          class="px-3 py-1 rounded text-[12px] border border-[var(--border)]
                 text-[var(--text-primary)] hover:border-[var(--accent)] transition-colors"
        >{p.label}</button>
      {/each}
    </div>
  </fieldset>

  <!-- Bitrate -->
  {#if !isLossless}
    <fieldset>
      <legend class="text-[12px] font-medium text-[var(--text-secondary)] uppercase tracking-wide mb-2">
        Bitrate
      </legend>
      <div class="flex flex-wrap gap-2">
        {#each bitrates as br}
          <button
            onclick={() => options.bitrate = br}
            class="px-3 py-1 rounded text-[12px] border transition-colors
              {options.bitrate === br
                ? 'bg-[var(--accent)] text-white border-[var(--accent)]'
                : 'border-[var(--border)] text-[var(--text-primary)] hover:border-[var(--accent)]'}"
          >{br} kbps</button>
        {/each}
      </div>
    </fieldset>
  {/if}

  <!-- Sample rate -->
  <fieldset>
    <legend class="text-[12px] font-medium text-[var(--text-secondary)] uppercase tracking-wide mb-2">
      Sample Rate
    </legend>
    <select
      bind:value={options.sample_rate}
      class="w-full px-3 py-1.5 rounded-md border border-[var(--border)]
             bg-[var(--surface)] text-[var(--text-primary)] text-[13px]
             focus:outline-none focus:border-[var(--accent)]"
    >
      {#each sampleRates as sr}
        <option value={sr.value}>{sr.label}</option>
      {/each}
    </select>
  </fieldset>

  <!-- Trim -->
  <fieldset>
    <legend class="text-[12px] font-medium text-[var(--text-secondary)] uppercase tracking-wide mb-2">
      Trim (MM:SS or seconds)
    </legend>
    <div class="flex gap-3 items-end">
      <div class="flex-1">
        <label class="text-[11px] text-[var(--text-secondary)]" for="aud-trim-start">Start</label>
        <input id="aud-trim-start" type="text" placeholder="0:00"
          bind:value={trimStartRaw}
          onchange={onTrimStartChange}
          class="w-full mt-1 px-3 py-1.5 rounded-md border border-[var(--border)]
                 bg-[var(--surface)] text-[var(--text-primary)] text-[13px]
                 focus:outline-none focus:border-[var(--accent)]"
        />
      </div>
      <div class="flex-1">
        <label class="text-[11px] text-[var(--text-secondary)]" for="aud-trim-end">End</label>
        <input id="aud-trim-end" type="text" placeholder="end"
          bind:value={trimEndRaw}
          onchange={onTrimEndChange}
          class="w-full mt-1 px-3 py-1.5 rounded-md text-[13px]
                 focus:outline-none focus:border-[var(--accent)]
                 bg-[var(--surface)] text-[var(--text-primary)]
                 {errors.audio_trim ? 'border border-red-500' : 'border border-[var(--border)]'}"
        />
      </div>
      {#if options.trim_start != null || options.trim_end != null}
        <button onclick={clearTrim}
          class="px-3 py-1.5 rounded-md text-[12px] border border-[var(--border)]
                 text-red-500 hover:border-red-400 transition-colors shrink-0">
          Clear
        </button>
      {/if}
    </div>
    {#if errors.audio_trim}
      <p class="text-[11px] text-red-500 mt-1">{errors.audio_trim}</p>
    {:else}
      <p class="text-[11px] text-[var(--text-secondary)] mt-1.5">Leave blank to keep full duration.</p>
    {/if}
  </fieldset>

  <!-- Loudness normalization -->
  <fieldset>
    <legend class="text-[12px] font-medium text-[var(--text-secondary)] uppercase tracking-wide mb-2">
      Processing
    </legend>
    <label class="flex items-start gap-2.5 cursor-pointer">
      <input
        type="checkbox"
        bind:checked={options.normalize_loudness}
        class="mt-0.5 accent-[var(--accent)]"
        aria-describedby="norm-desc"
      />
      <div>
        <span class="text-[13px] text-[var(--text-primary)]">Normalize loudness</span>
        <p id="norm-desc" class="text-[11px] text-[var(--text-secondary)] mt-0.5">
          EBU R128 — brings all tracks to a consistent volume level. Useful for podcasts, voice recordings, and mixed-source playlists.
        </p>
      </div>
    </label>
  </fieldset>

</div>
