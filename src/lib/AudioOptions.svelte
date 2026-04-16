<script>
  let { options = $bindable(), errors = {} } = $props();

  const quickFormats   = ['mp3', 'wav', 'flac'];
  const extendedFormats = ['ogg', 'aac', 'opus', 'm4a', 'wma', 'aiff', 'alac', 'ac3', 'dts'];
  const bitrates = [64, 128, 192, 256, 320];
  const sampleRates = [
    { value: 44100,  label: '44.1 kHz — CD' },
    { value: 48000,  label: '48 kHz — Video standard' },
    { value: 96000,  label: '96 kHz — Hi-Fi' },
    { value: 192000, label: '192 kHz — Archival' },
  ];
  const presets = [
    { label: 'Streaming',    fmt: 'mp3',  br: 192,  sr: 44100, norm: false },
    { label: 'Voice only',   fmt: 'mp3',  br: 64,   sr: 44100, norm: true  },
    { label: 'CD quality',   fmt: 'mp3',  br: 320,  sr: 44100, norm: false },
    { label: 'Lossless',     fmt: 'flac', br: null,  sr: 44100, norm: false },
    { label: 'Podcast',      fmt: 'mp3',  br: 128,  sr: 44100, norm: true  },
    { label: 'Opus (small)', fmt: 'opus', br: 96,   sr: 48000, norm: false },
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

  let trimStartRaw = $derived(options.trim_start != null ? formatTime(options.trim_start) : '');
  let trimEndRaw   = $derived(options.trim_end   != null ? formatTime(options.trim_end)   : '');

  function onTrimStartInput(e) { options.trim_start = parseTime(e.target.value); }
  function onTrimEndInput(e)   { options.trim_end   = parseTime(e.target.value); }
  function clearTrim() { options.trim_start = null; options.trim_end = null; }

  // ── Button style helpers ───────────────────────────────────────────────────

  // Horizontal connected row (same as VideoOptions)
  function seg(active, i, total) {
    const base  = 'px-3 py-1.5 text-center text-[12px] font-medium border transition-colors relative';
    const round = i === 0 ? 'rounded-l-md' : i === total - 1 ? 'rounded-r-md' : '';
    const ml    = i > 0 ? '-ml-px' : '';
    const color = active
      ? 'bg-[var(--accent)] text-white border-[var(--accent)] z-10'
      : 'border-[var(--border)] text-[var(--text-primary)] hover:z-10 hover:border-[var(--accent)] hover:text-[var(--accent)]';
    return [base, round, ml, color].filter(Boolean).join(' ');
  }

  // Vertical connected stack (for sample rate — full label text)
  function segV(active, i, total) {
    const base  = 'w-full px-3 py-1.5 text-left text-[12px] font-medium border transition-colors relative';
    const round = i === 0 ? 'rounded-t-md' : i === total - 1 ? 'rounded-b-md' : '';
    const mt    = i > 0 ? '-mt-px' : '';
    const color = active
      ? 'bg-[var(--accent)] text-white border-[var(--accent)] z-10'
      : 'border-[var(--border)] text-[var(--text-primary)] hover:z-10 hover:border-[var(--accent)] hover:text-[var(--accent)]';
    return [base, round, mt, color].filter(Boolean).join(' ');
  }

  // 2-D grid (N columns) — corners get their own rounding
  function segGrid(i, cols, total) {
    const row     = Math.floor(i / cols);
    const col     = i % cols;
    const rows    = Math.ceil(total / cols);
    const lastRow = row === rows - 1;
    const lastCol = col === cols - 1 || i === total - 1;
    let round = '';
    if (row === 0     && col === 0)    round = 'rounded-tl-md';
    if (row === 0     && lastCol)      round = 'rounded-tr-md';
    if (lastRow       && col === 0)    round = 'rounded-bl-md';
    if (lastRow       && lastCol)      round = 'rounded-br-md';
    const ml = col > 0 ? '-ml-px' : '';
    const mt = row > 0 ? '-mt-px' : '';
    return [
      'px-3 py-1.5 text-center text-[12px] font-medium border transition-colors relative',
      round, ml, mt,
      'border-[var(--border)] text-[var(--text-primary)] hover:z-10 hover:border-[var(--accent)] hover:text-[var(--accent)]',
    ].filter(Boolean).join(' ');
  }
</script>

<div class="space-y-5" role="form" aria-label="Audio conversion options">

  <!-- Output format -->
  <fieldset>
    <legend class="text-[12px] font-medium text-[var(--text-secondary)] uppercase tracking-wide mb-2">
      Output Format
    </legend>
    <div class="flex gap-2 items-center">
      <!-- Primary 3 — connected group -->
      <div class="flex-1 grid" style="grid-template-columns: repeat({quickFormats.length}, 1fr)">
        {#each quickFormats as fmt, i}
          <button
            onclick={() => options.output_format = fmt}
            class={seg(options.output_format === fmt, i, quickFormats.length)}
          >{fmt.toUpperCase()}</button>
        {/each}
      </div>
      <!-- Extended formats dropdown -->
      <select
        onchange={(e) => options.output_format = e.target.value}
        class="px-2 py-1 rounded text-[12px] border transition-colors
               bg-[var(--surface)] outline-none cursor-pointer
               {extendedFormats.includes(options.output_format)
                 ? 'border-[var(--accent)] text-[var(--accent)]'
                 : 'border-[var(--border)] text-[var(--text-secondary)]'}"
      >
        <option value="" disabled selected={!extendedFormats.includes(options.output_format)}>More…</option>
        {#each extendedFormats as fmt}
          <option value={fmt} selected={options.output_format === fmt}>{fmt.toUpperCase()}</option>
        {/each}
      </select>
    </div>
    {#if isLossless}
      <p class="text-[11px] text-[var(--text-secondary)] mt-1.5">Lossless — bitrate not applicable.</p>
    {/if}
  </fieldset>

  <!-- Quick presets — 3-column connected grid -->
  <fieldset>
    <legend class="text-[12px] font-medium text-[var(--text-secondary)] uppercase tracking-wide mb-2">
      Quick Presets
    </legend>
    <div class="grid grid-cols-3">
      {#each presets as p, i}
        <button
          onclick={() => applyPreset(p)}
          class={segGrid(i, 3, presets.length)}
        >{p.label}</button>
      {/each}
    </div>
  </fieldset>

  <!-- Bitrate — horizontal connected -->
  {#if !isLossless}
    <fieldset data-tooltip="64–128 kbps for voice/podcast · 192 kbps standard for music streaming · 320 kbps for archival MP3">
      <legend class="text-[12px] font-medium text-[var(--text-secondary)] uppercase tracking-wide mb-2">
        Bitrate — kbps
      </legend>
      <div class="grid" style="grid-template-columns: repeat({bitrates.length}, 1fr)">
        {#each bitrates as br, i}
          <button
            onclick={() => options.bitrate = br}
            class={seg(options.bitrate === br, i, bitrates.length)}
          >{br}</button>
        {/each}
      </div>
    </fieldset>
  {/if}

  <!-- Sample rate — vertical connected stack (full label text) -->
  <fieldset data-tooltip="44.1 kHz for music/CD · 48 kHz for video sync · 96/192 kHz for recording/archival">
    <legend class="text-[12px] font-medium text-[var(--text-secondary)] uppercase tracking-wide mb-2">
      Sample Rate
    </legend>
    <div class="flex flex-col">
      {#each sampleRates as sr, i}
        <button
          onclick={() => options.sample_rate = sr.value}
          class={segV(options.sample_rate === sr.value, i, sampleRates.length)}
        >{sr.label}</button>
      {/each}
    </div>
  </fieldset>

  <!-- Trim -->
  <fieldset>
    <legend class="text-[12px] font-medium text-[var(--text-secondary)] uppercase tracking-wide mb-2">
      Trim (MM:SS or seconds)
    </legend>
    <div class="flex gap-3 items-end">
      <div class="flex-1">
        <label class="text-[11px] text-[var(--text-secondary)]" for="aud-trim-start">Start</label>
        <input id="aud-trim-start" type="text"
          value={trimStartRaw} oninput={onTrimStartInput}
          class="w-full mt-1 px-3 py-1.5 rounded-md border border-[var(--border)]
                 bg-[var(--surface)] text-[var(--text-primary)] text-[13px]
                 focus:outline-none focus:border-[var(--accent)]"
        />
      </div>
      <div class="flex-1">
        <label class="text-[11px] text-[var(--text-secondary)]" for="aud-trim-end">End</label>
        <input id="aud-trim-end" type="text"
          value={trimEndRaw} oninput={onTrimEndInput}
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
    {/if}
  </fieldset>

  <!-- Processing -->
  <fieldset
    data-tooltip="Hard-limit peaks · EBU R128 normalisation · Butterworth highpass/lowpass · Stereo width adjustment"
  >
    <legend class="text-[12px] font-medium text-[var(--text-secondary)] uppercase tracking-wide mb-2">
      Processing
    </legend>

    <!-- Accordion-style DSP toggles: rounded, separated, chevron, inline expansion -->
    <div class="space-y-1">

      <!-- Limiter -->
      <div>
        <button onclick={() => options.dsp_limiter_db = options.dsp_limiter_db != null ? null : -1.0}
                class="w-full px-3 py-1.5 rounded-md text-left text-[12px] font-medium border flex items-center gap-2 transition-colors
                       {options.dsp_limiter_db != null ? 'bg-[var(--accent)] text-white border-[var(--accent)]' : 'border-[var(--border)] text-[var(--text-primary)] hover:border-[var(--accent)] hover:text-[var(--accent)]'}">
          <span class="flex-1">Limiter</span>
          {#if options.dsp_limiter_db != null}<span class="font-mono text-[11px]">{options.dsp_limiter_db.toFixed(1)} dBFS</span>{/if}
          <svg class="shrink-0 transition-transform duration-200 {options.dsp_limiter_db != null ? 'rotate-180' : ''}" width="10" height="6" viewBox="0 0 10 6" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"><path d="M1 1l4 4 4-4"/></svg>
        </button>
        {#if options.dsp_limiter_db != null}
          <div class="mt-1 px-1 space-y-1">
            <input type="range" min="-12" max="0" step="0.5" bind:value={options.dsp_limiter_db} class="w-full accent-[var(--accent)]" />
            <div class="flex justify-between text-[10px] text-[var(--text-secondary)]"><span>−12 dBFS</span><span>0 dBFS</span></div>
          </div>
        {/if}
      </div>

      <!-- Normalize (simple toggle, no sub-controls) -->
      <button onclick={() => options.normalize_loudness = !options.normalize_loudness}
              class="w-full px-3 py-1.5 rounded-md text-left text-[12px] font-medium border flex items-center gap-2 transition-colors
                     {options.normalize_loudness ? 'bg-[var(--accent)] text-white border-[var(--accent)]' : 'border-[var(--border)] text-[var(--text-primary)] hover:border-[var(--accent)] hover:text-[var(--accent)]'}">
        <span class="flex-1">Normalize</span>
        {#if options.normalize_loudness}<span class="font-mono text-[11px]">EBU R128</span>{/if}
      </button>

      <!-- Highpass -->
      <div>
        <button onclick={() => options.dsp_highpass_freq = options.dsp_highpass_freq != null ? null : 80}
                class="w-full px-3 py-1.5 rounded-md text-left text-[12px] font-medium border flex items-center gap-2 transition-colors
                       {options.dsp_highpass_freq != null ? 'bg-[var(--accent)] text-white border-[var(--accent)]' : 'border-[var(--border)] text-[var(--text-primary)] hover:border-[var(--accent)] hover:text-[var(--accent)]'}">
          <span class="flex-1">Highpass</span>
          {#if options.dsp_highpass_freq != null}<span class="font-mono text-[11px]">{options.dsp_highpass_freq} Hz</span>{/if}
          <svg class="shrink-0 transition-transform duration-200 {options.dsp_highpass_freq != null ? 'rotate-180' : ''}" width="10" height="6" viewBox="0 0 10 6" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"><path d="M1 1l4 4 4-4"/></svg>
        </button>
        {#if options.dsp_highpass_freq != null}
          <div class="mt-1 px-1 space-y-1.5">
            <div class="flex items-center gap-2">
              <input type="number" value={options.dsp_highpass_freq} oninput={(e) => options.dsp_highpass_freq = Math.max(1, parseFloat(e.target.value) || 80)} min="1" max="20000" step="1"
                     class="w-24 px-2 py-1 text-[12px] font-mono rounded border border-[var(--border)] bg-[var(--surface)] text-[var(--text-primary)] outline-none focus:border-[var(--accent)]" />
              <span class="text-[11px] text-[var(--text-secondary)]">Hz</span>
            </div>
            <input type="range" min="0" max="100" value={Math.max(0, Math.round(Math.log10(Math.max(1, options.dsp_highpass_freq) / 20) / 3 * 100))} oninput={(e) => options.dsp_highpass_freq = Math.round(20 * Math.pow(1000, parseFloat(e.target.value) / 100))} class="w-full accent-[var(--accent)]" />
            <div class="flex justify-between text-[10px] text-[var(--text-secondary)]"><span>20 Hz</span><span>630 Hz</span><span>20 kHz</span></div>
          </div>
        {/if}
      </div>

      <!-- Lowpass -->
      <div>
        <button onclick={() => options.dsp_lowpass_freq = options.dsp_lowpass_freq != null ? null : 8000}
                class="w-full px-3 py-1.5 rounded-md text-left text-[12px] font-medium border flex items-center gap-2 transition-colors
                       {options.dsp_lowpass_freq != null ? 'bg-[var(--accent)] text-white border-[var(--accent)]' : 'border-[var(--border)] text-[var(--text-primary)] hover:border-[var(--accent)] hover:text-[var(--accent)]'}">
          <span class="flex-1">Lowpass</span>
          {#if options.dsp_lowpass_freq != null}<span class="font-mono text-[11px]">{options.dsp_lowpass_freq} Hz</span>{/if}
          <svg class="shrink-0 transition-transform duration-200 {options.dsp_lowpass_freq != null ? 'rotate-180' : ''}" width="10" height="6" viewBox="0 0 10 6" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"><path d="M1 1l4 4 4-4"/></svg>
        </button>
        {#if options.dsp_lowpass_freq != null}
          <div class="mt-1 px-1 space-y-1.5">
            <div class="flex items-center gap-2">
              <input type="number" value={options.dsp_lowpass_freq} oninput={(e) => options.dsp_lowpass_freq = Math.max(1, parseFloat(e.target.value) || 8000)} min="1" max="20000" step="1"
                     class="w-24 px-2 py-1 text-[12px] font-mono rounded border border-[var(--border)] bg-[var(--surface)] text-[var(--text-primary)] outline-none focus:border-[var(--accent)]" />
              <span class="text-[11px] text-[var(--text-secondary)]">Hz</span>
            </div>
            <input type="range" min="0" max="100" value={Math.min(100, Math.max(0, Math.round(Math.log10(Math.max(1, options.dsp_lowpass_freq) / 20) / 3 * 100)))} oninput={(e) => options.dsp_lowpass_freq = Math.round(20 * Math.pow(1000, parseFloat(e.target.value) / 100))} class="w-full accent-[var(--accent)]" />
            <div class="flex justify-between text-[10px] text-[var(--text-secondary)]"><span>20 Hz</span><span>630 Hz</span><span>20 kHz</span></div>
          </div>
        {/if}
      </div>

      <!-- Stereo Width -->
      <div>
        <button onclick={() => options.dsp_stereo_width = options.dsp_stereo_width != null ? null : 0}
                class="w-full px-3 py-1.5 rounded-md text-left text-[12px] font-medium border flex items-center gap-2 transition-colors
                       {options.dsp_stereo_width != null ? 'bg-[var(--accent)] text-white border-[var(--accent)]' : 'border-[var(--border)] text-[var(--text-primary)] hover:border-[var(--accent)] hover:text-[var(--accent)]'}">
          <span class="flex-1">Stereo Width</span>
          {#if options.dsp_stereo_width != null}<span class="font-mono text-[11px]">{options.dsp_stereo_width > 0 ? '+' : ''}{Math.round(options.dsp_stereo_width)}%</span>{/if}
          <svg class="shrink-0 transition-transform duration-200 {options.dsp_stereo_width != null ? 'rotate-180' : ''}" width="10" height="6" viewBox="0 0 10 6" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"><path d="M1 1l4 4 4-4"/></svg>
        </button>
        {#if options.dsp_stereo_width != null}
          <div class="mt-1 px-1 space-y-1">
            <input type="range" min="-100" max="100" step="1" bind:value={options.dsp_stereo_width} class="w-full accent-[var(--accent)]" />
            <div class="flex justify-between text-[10px] text-[var(--text-secondary)]"><span>−100 Mono</span><span>0</span><span>+100 Wide</span></div>
          </div>
        {/if}
      </div>

    </div>

  </fieldset>

</div>
