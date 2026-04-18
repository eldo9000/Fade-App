<script>
  let { options = $bindable(), errors = {} } = $props();

  const bitrates = [64, 128, 192, 256, 320];
  const sampleRates = [
    { value: 44100,  label: '44.1 kHz — CD' },
    { value: 48000,  label: '48 kHz — Video standard' },
    { value: 96000,  label: '96 kHz — Hi-Fi' },
    { value: 192000, label: '192 kHz — Archival' },
  ];

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

  // Dev-only green segment helpers
  function devSeg(i, total) {
    const base  = 'px-3 py-1.5 text-center text-[12px] font-medium border transition-colors relative';
    const round = i === 0 ? 'rounded-l-md' : i === total - 1 ? 'rounded-r-md' : '';
    const ml    = i > 0 ? '-ml-px' : '';
    return [base, round, ml, 'border-green-900 text-green-400 hover:border-green-700 hover:bg-green-950/40'].filter(Boolean).join(' ');
  }
  function devSegV(i, total) {
    const base  = 'w-full px-3 py-1.5 text-left text-[12px] font-medium border transition-colors relative';
    const round = i === 0 ? 'rounded-t-md' : i === total - 1 ? 'rounded-b-md' : '';
    const mt    = i > 0 ? '-mt-px' : '';
    return [base, round, mt, 'border-green-900 text-green-400 hover:border-green-700 hover:bg-green-950/40'].filter(Boolean).join(' ');
  }

  // 2-D grid (N columns) — corners get their own rounding
  function segGrid(i, cols, total, active = false) {
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
    const color = active
      ? 'bg-[var(--accent)] text-white border-[var(--accent)] z-10'
      : 'border-[var(--border)] text-[var(--text-primary)] hover:z-10 hover:border-[var(--accent)] hover:text-[var(--accent)]';
    return [
      'px-3 py-1.5 text-center text-[12px] font-medium border transition-colors relative',
      round, ml, mt, color,
    ].filter(Boolean).join(' ');
  }
</script>

<div class="space-y-5" role="form" aria-label="Audio conversion options">

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

  <!-- ── Format-specific controls (dev) ──────────────────────────────────── -->
  {#if import.meta.env.DEV}
    <div class="flex items-center gap-2">
      <div class="flex-1 h-px bg-green-900/50"></div>
      <span class="text-[9px] text-green-500/70 uppercase tracking-widest font-mono shrink-0">format-specific · dev</span>
      <div class="flex-1 h-px bg-green-900/50"></div>
    </div>

    {#if options.output_format === 'mp3'}
      <fieldset>
        <legend class="text-[12px] font-medium text-green-400 uppercase tracking-wide mb-2">Bitrate Mode</legend>
        <div class="grid" style="grid-template-columns:repeat(2,1fr)">
          {#each ['CBR','VBR'] as m, i}<button class={devSeg(i,2)}>{m}</button>{/each}
        </div>
      </fieldset>
      <fieldset>
        <legend class="text-[12px] font-medium text-green-400 uppercase tracking-wide mb-2">VBR Quality</legend>
        <div class="grid" style="grid-template-columns:repeat(5,1fr)">
          {#each ['V0','V2','V4','V6','V9'] as v, i}<button class={devSeg(i,5)}>{v}</button>{/each}
        </div>
        <p class="text-[10px] text-green-500/60 mt-1">V0 best · V9 smallest</p>
      </fieldset>
      <fieldset>
        <legend class="text-[12px] font-medium text-green-400 uppercase tracking-wide mb-2">Channels</legend>
        <div class="grid" style="grid-template-columns:repeat(3,1fr)">
          {#each ['Mono','Stereo','Joint Stereo'] as ch, i}<button class={devSeg(i,3)}>{ch}</button>{/each}
        </div>
      </fieldset>

    {:else if options.output_format === 'wav' || options.output_format === 'aiff'}
      <fieldset>
        <legend class="text-[12px] font-medium text-green-400 uppercase tracking-wide mb-2">Bit Depth</legend>
        <div class="grid" style="grid-template-columns:repeat(3,1fr)">
          {#each ['16-bit','24-bit','32-bit float'] as d, i}<button class={devSeg(i,3)}>{d}</button>{/each}
        </div>
      </fieldset>
      <fieldset>
        <legend class="text-[12px] font-medium text-green-400 uppercase tracking-wide mb-2">Channels</legend>
        <div class="grid" style="grid-template-columns:repeat(2,1fr)">
          {#each ['Mono','Stereo'] as ch, i}<button class={devSeg(i,2)}>{ch}</button>{/each}
        </div>
      </fieldset>

    {:else if options.output_format === 'flac'}
      <fieldset>
        <legend class="text-[12px] font-medium text-green-400 uppercase tracking-wide mb-2">Compression Level</legend>
        <div class="grid" style="grid-template-columns:repeat(5,1fr)">
          {#each [0,2,5,6,8] as l, i}<button class={devSeg(i,5)}>{l}</button>{/each}
        </div>
        <p class="text-[10px] text-green-500/60 mt-1">0 fastest · 8 smallest · no quality difference</p>
      </fieldset>
      <fieldset>
        <legend class="text-[12px] font-medium text-green-400 uppercase tracking-wide mb-2">Bit Depth</legend>
        <div class="grid" style="grid-template-columns:repeat(2,1fr)">
          {#each ['16-bit','24-bit'] as d, i}<button class={devSeg(i,2)}>{d}</button>{/each}
        </div>
      </fieldset>
      <fieldset>
        <legend class="text-[12px] font-medium text-green-400 uppercase tracking-wide mb-2">Channels</legend>
        <div class="grid" style="grid-template-columns:repeat(3,1fr)">
          {#each ['Mono','Stereo','Multi'] as ch, i}<button class={devSeg(i,3)}>{ch}</button>{/each}
        </div>
      </fieldset>

    {:else if options.output_format === 'ogg'}
      <fieldset>
        <legend class="text-[12px] font-medium text-green-400 uppercase tracking-wide mb-2">Bitrate Mode</legend>
        <div class="grid" style="grid-template-columns:repeat(3,1fr)">
          {#each ['VBR','CBR','ABR'] as m, i}<button class={devSeg(i,3)}>{m}</button>{/each}
        </div>
      </fieldset>
      <fieldset>
        <legend class="text-[12px] font-medium text-green-400 uppercase tracking-wide mb-2">VBR Quality</legend>
        <div class="grid" style="grid-template-columns:repeat(5,1fr)">
          {#each [-1,1,3,6,10] as q, i}<button class={devSeg(i,5)}>{q}</button>{/each}
        </div>
        <p class="text-[10px] text-green-500/60 mt-1">−1 lowest · 10 highest · 3–6 typical</p>
      </fieldset>

    {:else if options.output_format === 'aac'}
      <fieldset>
        <legend class="text-[12px] font-medium text-green-400 uppercase tracking-wide mb-2">AAC Profile</legend>
        <div class="flex flex-col">
          {#each ['AAC-LC','HE-AAC','HE-AACv2'] as p, i}<button class={devSegV(i,3)}>{p}</button>{/each}
        </div>
        <p class="text-[10px] text-green-500/60 mt-1">HE-AAC efficient at ≤128 kbps · HE-AACv2 adds Parametric Stereo</p>
      </fieldset>
      <fieldset>
        <legend class="text-[12px] font-medium text-green-400 uppercase tracking-wide mb-2">Channels</legend>
        <div class="grid" style="grid-template-columns:repeat(2,1fr)">
          {#each ['Mono','Stereo'] as ch, i}<button class={devSeg(i,2)}>{ch}</button>{/each}
        </div>
      </fieldset>

    {:else if options.output_format === 'opus'}
      <fieldset>
        <legend class="text-[12px] font-medium text-green-400 uppercase tracking-wide mb-2">Application Mode</legend>
        <div class="grid" style="grid-template-columns:repeat(3,1fr)">
          {#each ['Music','Voice','Low-Delay'] as m, i}<button class={devSeg(i,3)}>{m}</button>{/each}
        </div>
      </fieldset>
      <fieldset>
        <legend class="text-[12px] font-medium text-green-400 uppercase tracking-wide mb-2">Channels</legend>
        <div class="grid" style="grid-template-columns:repeat(2,1fr)">
          {#each ['Mono','Stereo'] as ch, i}<button class={devSeg(i,2)}>{ch}</button>{/each}
        </div>
        <p class="text-[10px] text-green-500/60 mt-1">Always 48 kHz — Opus resamples internally</p>
      </fieldset>

    {:else if options.output_format === 'm4a'}
      <fieldset>
        <legend class="text-[12px] font-medium text-green-400 uppercase tracking-wide mb-2">Sub-Codec</legend>
        <div class="grid" style="grid-template-columns:repeat(2,1fr)">
          {#each ['AAC (lossy)','ALAC (lossless)'] as c, i}<button class={devSeg(i,2)}>{c}</button>{/each}
        </div>
      </fieldset>
      <fieldset>
        <legend class="text-[12px] font-medium text-green-400 uppercase tracking-wide mb-2">Bit Depth (ALAC)</legend>
        <div class="grid" style="grid-template-columns:repeat(2,1fr)">
          {#each ['16-bit','24-bit'] as d, i}<button class={devSeg(i,2)}>{d}</button>{/each}
        </div>
      </fieldset>

    {:else if options.output_format === 'wma'}
      <fieldset>
        <legend class="text-[12px] font-medium text-green-400 uppercase tracking-wide mb-2">WMA Mode</legend>
        <div class="flex flex-col">
          {#each ['Standard','Pro (multi-channel)','Lossless'] as m, i}<button class={devSegV(i,3)}>{m}</button>{/each}
        </div>
      </fieldset>

    {:else if options.output_format === 'alac'}
      <fieldset>
        <legend class="text-[12px] font-medium text-green-400 uppercase tracking-wide mb-2">Bit Depth</legend>
        <div class="grid" style="grid-template-columns:repeat(3,1fr)">
          {#each ['16-bit','24-bit','32-bit'] as d, i}<button class={devSeg(i,3)}>{d}</button>{/each}
        </div>
      </fieldset>
      <fieldset>
        <legend class="text-[12px] font-medium text-green-400 uppercase tracking-wide mb-2">Channels</legend>
        <div class="grid" style="grid-template-columns:repeat(3,1fr)">
          {#each ['Mono','Stereo','Multi'] as ch, i}<button class={devSeg(i,3)}>{ch}</button>{/each}
        </div>
      </fieldset>

    {:else if options.output_format === 'ac3'}
      <fieldset>
        <legend class="text-[12px] font-medium text-green-400 uppercase tracking-wide mb-2">Channels</legend>
        <div class="grid" style="grid-template-columns:repeat(3,1fr)">
          {#each ['Mono','Stereo','5.1'] as ch, i}<button class={devSeg(i,3)}>{ch}</button>{/each}
        </div>
      </fieldset>
      <fieldset>
        <legend class="text-[12px] font-medium text-green-400 uppercase tracking-wide mb-2">Bitrate</legend>
        <div class="grid" style="grid-template-columns:repeat(3,1fr)">
          {#each ['192 kbps','384 kbps','640 kbps'] as br, i}<button class={devSeg(i,3)}>{br}</button>{/each}
        </div>
        <p class="text-[10px] text-green-500/60 mt-1">448 kbps typical for 5.1 broadcast · 48 kHz required</p>
      </fieldset>
      <fieldset>
        <legend class="text-[12px] font-medium text-green-400 uppercase tracking-wide mb-2">Dialogue Normalization</legend>
        <div class="flex items-center gap-3">
          <input type="range" min="-31" max="-1" value="-31" disabled class="flex-1 accent-green-600 opacity-60" />
          <span class="text-[11px] text-green-400 font-mono shrink-0">−31 dBFS</span>
        </div>
      </fieldset>

    {:else if options.output_format === 'dts'}
      <fieldset>
        <legend class="text-[12px] font-medium text-green-400 uppercase tracking-wide mb-2">Channels</legend>
        <div class="grid" style="grid-template-columns:repeat(2,1fr)">
          {#each ['Stereo','5.1'] as ch, i}<button class={devSeg(i,2)}>{ch}</button>{/each}
        </div>
      </fieldset>
      <fieldset>
        <legend class="text-[12px] font-medium text-green-400 uppercase tracking-wide mb-2">Bitrate</legend>
        <div class="grid" style="grid-template-columns:repeat(2,1fr)">
          {#each ['754 kbps','1510 kbps'] as br, i}<button class={devSeg(i,2)}>{br}</button>{/each}
        </div>
        <p class="text-[10px] text-green-500/60 mt-1">FFmpeg encodes DTS core only — usually passthrough</p>
      </fieldset>
    {/if}
  {/if}

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

      <!-- Normalize -->
      <div>
        <button onclick={() => { options.normalize_loudness = !options.normalize_loudness; if (options.normalize_loudness) { options.normalize_lufs ??= -16; options.normalize_true_peak ??= -1; } }}
                class="w-full px-3 py-1.5 rounded-md text-left text-[12px] font-medium border flex items-center gap-2 transition-colors
                       {options.normalize_loudness ? 'bg-[var(--accent)] text-white border-[var(--accent)]' : 'border-[var(--border)] text-[var(--text-primary)] hover:border-[var(--accent)] hover:text-[var(--accent)]'}">
          <span class="flex-1">Normalize</span>
          {#if options.normalize_loudness}
            <span class="font-mono text-[11px]">{options.normalize_lufs ?? -16} LUFS / {options.normalize_true_peak ?? -1} dBTP</span>
          {/if}
          <svg class="shrink-0 transition-transform duration-200 {options.normalize_loudness ? 'rotate-180' : ''}" width="10" height="6" viewBox="0 0 10 6" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"><path d="M1 1l4 4 4-4"/></svg>
        </button>
        {#if options.normalize_loudness}
          <div class="mt-1 px-1 space-y-2">
            <!-- LUFS target presets -->
            <div class="space-y-1">
              <div class="text-[10px] text-[var(--text-secondary)] px-1">Target LUFS</div>
              <div class="flex">
                {#each [[-23,'Broadcast'],[-16,'Streaming'],[-14,'Streaming+'],[-9,'Podcast']] as [val, label], i}
                  <button onclick={() => options.normalize_lufs = val}
                          class={segGrid(i, 4, 4, options.normalize_lufs === val)}>
                    <div class="font-mono text-[10px] leading-tight">{val}</div>
                    <div class="text-[9px] leading-tight opacity-70">{label}</div>
                  </button>
                {/each}
              </div>
              <div class="flex items-center gap-2 pt-0.5">
                <input type="number" value={options.normalize_lufs ?? -16}
                       oninput={(e) => options.normalize_lufs = Math.max(-70, Math.min(-1, parseFloat(e.target.value) || -16))}
                       min="-70" max="-1" step="0.5"
                       class="w-24 px-2 py-1 text-[12px] font-mono rounded border border-[var(--border)] bg-[var(--surface)] text-[var(--text-primary)] outline-none focus:border-[var(--accent)]" />
                <span class="text-[11px] text-[var(--text-secondary)]">LUFS</span>
              </div>
            </div>
            <!-- True peak ceiling -->
            <div class="space-y-1">
              <div class="text-[10px] text-[var(--text-secondary)] px-1">True Peak Ceiling</div>
              <input type="range" min="-6" max="-0.1" step="0.1"
                     value={options.normalize_true_peak ?? -1}
                     oninput={(e) => options.normalize_true_peak = parseFloat(e.target.value)}
                     class="w-full accent-[var(--accent)]" />
              <div class="flex justify-between items-center">
                <div class="flex justify-between text-[10px] text-[var(--text-secondary)] flex-1"><span>−6 dBTP</span><span>−0.1 dBTP</span></div>
                <span class="font-mono text-[11px] text-[var(--text-primary)] ml-2">{(options.normalize_true_peak ?? -1).toFixed(1)}</span>
              </div>
            </div>
          </div>
        {/if}
      </div>

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
