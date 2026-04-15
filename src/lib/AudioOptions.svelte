<script>
  let { options = $bindable(), errors = {} } = $props();

  const quickFormats = ['mp3', 'wav', 'flac'];
  const extendedFormats = ['ogg', 'aac', 'opus', 'm4a', 'wma', 'aiff', 'alac', 'ac3', 'dts'];
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

  let trimStartRaw = $derived(options.trim_start != null ? formatTime(options.trim_start) : '');
  let trimEndRaw   = $derived(options.trim_end   != null ? formatTime(options.trim_end)   : '');

  function onTrimStartInput(e) { options.trim_start = parseTime(e.target.value); }
  function onTrimEndInput(e)   { options.trim_end   = parseTime(e.target.value); }
  function clearTrim() {
    options.trim_start = null; options.trim_end = null;
  }
</script>

<div class="space-y-5" role="form" aria-label="Audio conversion options">

  <!-- Output format -->
  <fieldset>
    <legend class="text-[12px] font-medium text-[var(--text-secondary)] uppercase tracking-wide mb-2">
      Output Format
    </legend>
    <div class="flex gap-2">
      {#each quickFormats as fmt}
        <button
          onclick={() => options.output_format = fmt}
          class="px-3 py-1 rounded text-[12px] font-medium border transition-colors
            {options.output_format === fmt
              ? 'bg-[var(--accent)] text-white border-[var(--accent)]'
              : 'border-[var(--border)] text-[var(--text-primary)] hover:border-[var(--accent)]'}"
        >{fmt.toUpperCase()}</button>
      {/each}
      <select
        onchange={(e) => options.output_format = e.target.value}
        class="flex-1 px-2 py-1 rounded text-[12px] border transition-colors
               bg-[var(--surface)] text-[var(--text-primary)] outline-none
               focus:border-[var(--accent)]
               {extendedFormats.includes(options.output_format)
                 ? 'border-[var(--accent)] text-[var(--accent)]'
                 : 'border-[var(--border)]'}"
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
          value={trimStartRaw}
          oninput={onTrimStartInput}
          class="w-full mt-1 px-3 py-1.5 rounded-md border border-[var(--border)]
                 bg-[var(--surface)] text-[var(--text-primary)] text-[13px]
                 focus:outline-none focus:border-[var(--accent)]"
        />
      </div>
      <div class="flex-1">
        <label class="text-[11px] text-[var(--text-secondary)]" for="aud-trim-end">End</label>
        <input id="aud-trim-end" type="text" placeholder="end"
          value={trimEndRaw}
          oninput={onTrimEndInput}
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

  <!-- Processing -->
  <fieldset>
    <legend class="text-[12px] font-medium text-[var(--text-secondary)] uppercase tracking-wide mb-2">
      Processing
    </legend>
    <div class="space-y-3">

      <!-- Normalize loudness -->
      <label class="flex items-center gap-2.5 cursor-pointer">
        <input type="checkbox" bind:checked={options.normalize_loudness}
               class="accent-[var(--accent)]" />
        <span class="text-[13px] text-[var(--text-primary)]">Normalize loudness</span>
        <span class="text-[11px] text-[var(--text-secondary)] ml-auto">EBU R128</span>
      </label>

      <!-- Highpass filter -->
      <div>
        <label class="flex items-center gap-2.5 cursor-pointer">
          <input type="checkbox"
                 checked={options.dsp_highpass_freq != null}
                 onchange={(e) => options.dsp_highpass_freq = e.target.checked ? 80 : null}
                 class="accent-[var(--accent)]" />
          <span class="text-[13px] text-[var(--text-primary)]">Highpass</span>
          <span class="text-[11px] text-[var(--text-secondary)] ml-auto">Butterworth 2-pole</span>
        </label>
        {#if options.dsp_highpass_freq != null}
          <div class="mt-1.5 ml-6 space-y-1.5">
            <div class="flex items-center gap-2">
              <input type="number"
                     value={options.dsp_highpass_freq}
                     oninput={(e) => options.dsp_highpass_freq = Math.max(1, parseFloat(e.target.value) || 80)}
                     min="1" max="20000" step="1"
                     class="w-24 px-2 py-1 text-[12px] font-mono rounded border border-[var(--border)]
                            bg-[var(--surface)] text-[var(--text-primary)] outline-none
                            focus:border-[var(--accent)]" />
              <span class="text-[11px] text-[var(--text-secondary)]">Hz</span>
            </div>
            <input type="range" min="0" max="100"
                   value={Math.max(0, Math.round(Math.log10(Math.max(1, options.dsp_highpass_freq) / 20) / 3 * 100))}
                   oninput={(e) => options.dsp_highpass_freq = Math.round(20 * Math.pow(1000, parseFloat(e.target.value) / 100))}
                   class="w-full accent-[var(--accent)]" />
            <div class="flex justify-between text-[10px] text-[var(--text-secondary)]">
              <span>20 Hz</span><span>630 Hz</span><span>20 kHz</span>
            </div>
          </div>
        {/if}
      </div>

      <!-- Lowpass filter -->
      <div>
        <label class="flex items-center gap-2.5 cursor-pointer">
          <input type="checkbox"
                 checked={options.dsp_lowpass_freq != null}
                 onchange={(e) => options.dsp_lowpass_freq = e.target.checked ? 8000 : null}
                 class="accent-[var(--accent)]" />
          <span class="text-[13px] text-[var(--text-primary)]">Lowpass</span>
          <span class="text-[11px] text-[var(--text-secondary)] ml-auto">Butterworth 2-pole</span>
        </label>
        {#if options.dsp_lowpass_freq != null}
          <div class="mt-1.5 ml-6 space-y-1.5">
            <div class="flex items-center gap-2">
            <input type="number"
                   value={options.dsp_lowpass_freq}
                   oninput={(e) => options.dsp_lowpass_freq = Math.max(1, parseFloat(e.target.value) || 8000)}
                   min="1" max="20000" step="1"
                   class="w-24 px-2 py-1 text-[12px] font-mono rounded border border-[var(--border)]
                          bg-[var(--surface)] text-[var(--text-primary)] outline-none
                          focus:border-[var(--accent)]" />
              <span class="text-[11px] text-[var(--text-secondary)]">Hz</span>
            </div>
            <input type="range" min="0" max="100"
                   value={Math.min(100, Math.max(0, Math.round(Math.log10(Math.max(1, options.dsp_lowpass_freq) / 20) / 3 * 100)))}
                   oninput={(e) => options.dsp_lowpass_freq = Math.round(20 * Math.pow(1000, parseFloat(e.target.value) / 100))}
                   class="w-full accent-[var(--accent)]" />
            <div class="flex justify-between text-[10px] text-[var(--text-secondary)]">
              <span>20 Hz</span><span>630 Hz</span><span>20 kHz</span>
            </div>
          </div>
        {/if}
      </div>

      <!-- Stereo width -->
      <div>
        <label class="flex items-center gap-2.5 cursor-pointer">
          <input type="checkbox"
                 checked={options.dsp_stereo_width != null}
                 onchange={(e) => options.dsp_stereo_width = e.target.checked ? 1.5 : null}
                 class="accent-[var(--accent)]" />
          <span class="text-[13px] text-[var(--text-primary)]">Stereo width</span>
          {#if options.dsp_stereo_width != null}
            <span class="text-[11px] text-[var(--text-secondary)] ml-auto font-mono">
              {Math.round(options.dsp_stereo_width * 100)}%
            </span>
          {/if}
        </label>
        {#if options.dsp_stereo_width != null}
          <div class="mt-1.5 ml-6 space-y-1">
            <input type="range" min="0" max="2" step="0.05"
                   bind:value={options.dsp_stereo_width}
                   class="w-full accent-[var(--accent)]" />
            <div class="flex justify-between text-[10px] text-[var(--text-secondary)]">
              <span>Mono</span><span>Original</span><span>Wide</span>
            </div>
          </div>
        {/if}
      </div>

      <!-- Limiter -->
      <div>
        <label class="flex items-center gap-2.5 cursor-pointer">
          <input type="checkbox"
                 checked={options.dsp_limiter_db != null}
                 onchange={(e) => options.dsp_limiter_db = e.target.checked ? -1 : null}
                 class="accent-[var(--accent)]" />
          <span class="text-[13px] text-[var(--text-primary)]">Limiter</span>
          {#if options.dsp_limiter_db != null}
            <span class="text-[11px] text-[var(--text-secondary)] ml-auto font-mono">
              {options.dsp_limiter_db.toFixed(1)} dBFS
            </span>
          {/if}
        </label>
        {#if options.dsp_limiter_db != null}
          <div class="mt-1.5 ml-6 space-y-1">
            <input type="range" min="-12" max="0" step="0.5"
                   bind:value={options.dsp_limiter_db}
                   class="w-full accent-[var(--accent)]" />
            <div class="flex justify-between text-[10px] text-[var(--text-secondary)]">
              <span>−12 dBFS</span><span>0 dBFS</span>
            </div>
          </div>
        {/if}
      </div>

    </div>
  </fieldset>

</div>
