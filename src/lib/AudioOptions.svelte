<script>
  import SilencePad from './SilencePad.svelte';
  import { seg, segV } from './segStyles.js';
  let { options = $bindable(), errors = {} } = $props();

  const bitrates = [64, 128, 192, 256, 320];
  const sampleRates = [
    { value: 44100,  label: '44.1 kHz — CD' },
    { value: 48000,  label: '48 kHz — Video standard' },
    { value: 96000,  label: '96 kHz — Hi-Fi' },
    { value: 192000, label: '192 kHz — Archival' },
  ];

  const isLossless = $derived(['flac','wav','aiff','alac'].includes(options.output_format));
  // m4a is lossless when ALAC sub-codec selected
  const m4aIsLossless = $derived(options.output_format === 'm4a' && options.m4a_subcodec === 'alac');
  const hideBitrate = $derived(isLossless || m4aIsLossless || options.output_format === 'ac3' || options.output_format === 'dts' || (options.output_format === 'mp3' && options.mp3_bitrate_mode === 'vbr') || (options.output_format === 'ogg' && options.ogg_bitrate_mode === 'vbr'));

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

  // Dev-only green segment helpers
  function devSeg(i, total) {
    const base  = 'px-3 py-[5px] text-center text-[12px] font-medium border transition-colors relative';
    const round = i === 0 ? 'rounded-l-md' : i === total - 1 ? 'rounded-r-md' : '';
    const ml    = i > 0 ? '-ml-px' : '';
    return [base, round, ml, 'border-green-900 text-green-400 hover:border-green-700 hover:bg-green-950/40'].filter(Boolean).join(' ');
  }
  function devSegV(i, total) {
    const base  = 'w-full px-3 py-[5px] text-left text-[12px] font-medium border transition-colors relative';
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
      ? 'seg-active z-10'
      : 'seg-inactive border-[var(--border)] text-[color-mix(in_srgb,var(--text-primary)_70%,transparent)] hover:z-10';
    return [
      'px-3 py-[5px] text-center text-[12px] font-medium border transition-colors relative',
      round, ml, mt, color,
    ].filter(Boolean).join(' ');
  }
</script>

<div class="space-y-3" role="form" aria-label="Audio conversion options">

  <!-- Bitrate — horizontal connected -->
  {#if !hideBitrate}
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
    <div class="inline-flex flex-col">
      {#each sampleRates as sr, i}
        <button
          onclick={() => options.sample_rate = sr.value}
          class={segV(options.sample_rate === sr.value, i, sampleRates.length)}
        >{sr.label}</button>
      {/each}
    </div>
  </fieldset>

  <!-- Trim -->
  <fieldset data-tooltip="Trim the output — enter time as MM:SS or raw seconds. Leave blank to keep that edge. Visual trim handles on the timeline sync to these fields.">
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

  <fieldset data-tooltip="Prepend and/or append silence to the output. Useful for breathing room or aligning to a beat grid.">
    <legend class="text-[12px] font-medium text-[var(--text-secondary)] uppercase tracking-wide mb-2">Silence Padding</legend>
    <SilencePad bind:padFront={options.pad_front} bind:padEnd={options.pad_end} />
  </fieldset>

  <!-- ── Format-specific controls ──────────────────────────────────────── -->

  {#if options.output_format === 'mp3'}
    <fieldset data-tooltip="CBR — fixed bitrate · VBR — variable bitrate (smaller files at same quality)">
      <legend class="text-[12px] font-medium text-[var(--text-secondary)] uppercase tracking-wide mb-2">Bitrate Mode</legend>
      <div class="grid" style="grid-template-columns:repeat(2,1fr)">
        {#each [['cbr','CBR'],['vbr','VBR']] as [v, lbl], i}
          <button onclick={() => options.mp3_bitrate_mode = v} class={seg(options.mp3_bitrate_mode === v, i, 2)}>{lbl}</button>
        {/each}
      </div>
    </fieldset>
    {#if options.mp3_bitrate_mode === 'vbr'}
      <fieldset data-tooltip="LAME VBR quality: 0 ≈ 245 kbps avg · 4 ≈ 165 kbps · 9 ≈ 65 kbps">
        <legend class="text-[12px] font-medium text-[var(--text-secondary)] uppercase tracking-wide mb-2">VBR Quality — V{options.mp3_vbr_quality}</legend>
        <input type="range" min="0" max="9" step="1" bind:value={options.mp3_vbr_quality} class="fade-range"
               style="--fade-range-pct:{((options.mp3_vbr_quality ?? 0) / 9) * 100}%" />
        <div class="flex justify-between text-[10px] text-[var(--text-secondary)] mt-1"><span>V0 best</span><span>V9 smallest</span></div>
      </fieldset>
    {/if}
    <fieldset data-tooltip="Source — match input · Mono — single channel, smallest · Stereo — two channels · Joint — stereo with shared bits (MP3 only) · 5.1 — surround">
      <legend class="text-[12px] font-medium text-[var(--text-secondary)] uppercase tracking-wide mb-2">Channels</legend>
      <div class="grid" style="grid-template-columns:repeat(4,1fr)">
        {#each [['source','Source'],['mono','Mono'],['stereo','Stereo'],['joint','Joint']] as [v, lbl], i}
          <button onclick={() => options.channels = v} class={seg(options.channels === v, i, 4)}>{lbl}</button>
        {/each}
      </div>
    </fieldset>

  {:else if options.output_format === 'wav' || options.output_format === 'aiff'}
    <fieldset data-tooltip="16-bit CD-quality · 24-bit studio · 32-bit float for mixing / processing without clipping">
      <legend class="text-[12px] font-medium text-[var(--text-secondary)] uppercase tracking-wide mb-2">Bit Depth</legend>
      <div class="grid" style="grid-template-columns:repeat(3,1fr)">
        {#each [[16,'16-bit'],[24,'24-bit'],[32,'32-bit float']] as [v, lbl], i}
          <button onclick={() => options.bit_depth = v} class={seg(options.bit_depth === v, i, 3)}>{lbl}</button>
        {/each}
      </div>
    </fieldset>
    <fieldset data-tooltip="Source — match input · Mono — single channel, smallest · Stereo — two channels · Joint — stereo with shared bits (MP3 only) · 5.1 — surround">
      <legend class="text-[12px] font-medium text-[var(--text-secondary)] uppercase tracking-wide mb-2">Channels</legend>
      <div class="grid" style="grid-template-columns:repeat(3,1fr)">
        {#each [['source','Source'],['mono','Mono'],['stereo','Stereo']] as [v, lbl], i}
          <button onclick={() => options.channels = v} class={seg(options.channels === v, i, 3)}>{lbl}</button>
        {/each}
      </div>
    </fieldset>

  {:else if options.output_format === 'flac'}
    <fieldset data-tooltip="FLAC compression level — 0 fastest · 8 smallest · fully lossless at every level">
      <legend class="text-[12px] font-medium text-[var(--text-secondary)] uppercase tracking-wide mb-2">Compression — {options.flac_compression}</legend>
      <input type="range" min="0" max="8" step="1" bind:value={options.flac_compression} class="fade-range"
             style="--fade-range-pct:{((options.flac_compression ?? 0) / 8) * 100}%" />
      <div class="flex justify-between text-[10px] text-[var(--text-secondary)] mt-1"><span>0 fastest</span><span>8 smallest</span></div>
    </fieldset>
    <fieldset data-tooltip="16-bit CD-quality · 24-bit studio · 32-bit float for mixing / processing without clipping">
      <legend class="text-[12px] font-medium text-[var(--text-secondary)] uppercase tracking-wide mb-2">Bit Depth</legend>
      <div class="grid" style="grid-template-columns:repeat(2,1fr)">
        {#each [[16,'16-bit'],[24,'24-bit']] as [v, lbl], i}
          <button onclick={() => options.bit_depth = v} class={seg(options.bit_depth === v, i, 2)}>{lbl}</button>
        {/each}
      </div>
    </fieldset>

  {:else if options.output_format === 'ogg'}
    <fieldset data-tooltip="VBR — variable bitrate by quality setting · CBR — fixed bitrate · ABR — averages to a target">
      <legend class="text-[12px] font-medium text-[var(--text-secondary)] uppercase tracking-wide mb-2">Bitrate Mode</legend>
      <div class="grid" style="grid-template-columns:repeat(3,1fr)">
        {#each [['vbr','VBR'],['cbr','CBR'],['abr','ABR']] as [v, lbl], i}
          <button onclick={() => options.ogg_bitrate_mode = v} class={seg(options.ogg_bitrate_mode === v, i, 3)}>{lbl}</button>
        {/each}
      </div>
    </fieldset>
    {#if options.ogg_bitrate_mode === 'vbr'}
      <fieldset data-tooltip="Vorbis quality: -1 lowest · 10 highest · 3–6 typical">
        <legend class="text-[12px] font-medium text-[var(--text-secondary)] uppercase tracking-wide mb-2">Quality — {options.ogg_vbr_quality}</legend>
        <input type="range" min="-1" max="10" step="1" bind:value={options.ogg_vbr_quality} class="fade-range"
               style="--fade-range-pct:{(((options.ogg_vbr_quality ?? -1) - -1) / 11) * 100}%" />
        <div class="flex justify-between text-[10px] text-[var(--text-secondary)] mt-1"><span>-1 lowest</span><span>10 highest</span></div>
      </fieldset>
    {/if}

  {:else if options.output_format === 'aac'}
    <fieldset data-tooltip="LC universal · HE efficient ≤128 kbps · HEv2 adds Parametric Stereo for very low bitrates">
      <legend class="text-[12px] font-medium text-[var(--text-secondary)] uppercase tracking-wide mb-2">Profile</legend>
      <div class="inline-flex flex-col">
        {#each [['lc','AAC-LC'],['he','HE-AAC'],['hev2','HE-AACv2']] as [v, lbl], i}
          <button onclick={() => options.aac_profile = v} class={segV(options.aac_profile === v, i, 3)}>{lbl}</button>
        {/each}
      </div>
    </fieldset>
    <fieldset data-tooltip="Source — match input · Mono — single channel, smallest · Stereo — two channels · Joint — stereo with shared bits (MP3 only) · 5.1 — surround">
      <legend class="text-[12px] font-medium text-[var(--text-secondary)] uppercase tracking-wide mb-2">Channels</legend>
      <div class="grid" style="grid-template-columns:repeat(3,1fr)">
        {#each [['source','Source'],['mono','Mono'],['stereo','Stereo']] as [v, lbl], i}
          <button onclick={() => options.channels = v} class={seg(options.channels === v, i, 3)}>{lbl}</button>
        {/each}
      </div>
    </fieldset>

  {:else if options.output_format === 'opus'}
    <fieldset data-tooltip="audio — music · voip — speech · lowdelay — realtime">
      <legend class="text-[12px] font-medium text-[var(--text-secondary)] uppercase tracking-wide mb-2">Application</legend>
      <div class="grid" style="grid-template-columns:repeat(3,1fr)">
        {#each [['audio','Music'],['voip','Voice'],['lowdelay','Low-Delay']] as [v, lbl], i}
          <button onclick={() => options.opus_application = v} class={seg(options.opus_application === v, i, 3)}>{lbl}</button>
        {/each}
      </div>
    </fieldset>
    <label class="inline-flex items-center gap-2.5 cursor-pointer text-[13px]
                  bg-[var(--surface-hint)] border border-[var(--border)] rounded-md px-3 py-2
                  {options.opus_vbr ? 'text-[var(--text-primary)]' : 'text-white/75'}"
           data-tooltip="Opus VBR — allocates more bits to complex passages for better quality at the same average size. Uncheck for fixed bitrate.">
      <input type="checkbox" bind:checked={options.opus_vbr} class="fade-check" />
      Variable bitrate (VBR)
    </label>
    <fieldset data-tooltip="Source — match input · Mono — single channel, smallest · Stereo — two channels · Joint — stereo with shared bits (MP3 only) · 5.1 — surround">
      <legend class="text-[12px] font-medium text-[var(--text-secondary)] uppercase tracking-wide mb-2">Channels</legend>
      <div class="grid" style="grid-template-columns:repeat(3,1fr)">
        {#each [['source','Source'],['mono','Mono'],['stereo','Stereo']] as [v, lbl], i}
          <button onclick={() => options.channels = v} class={seg(options.channels === v, i, 3)}>{lbl}</button>
        {/each}
      </div>
    </fieldset>

  {:else if options.output_format === 'm4a'}
    <fieldset data-tooltip="AAC — small lossy files, universal compatibility · ALAC — Apple Lossless, same size as FLAC with native iOS/macOS support">
      <legend class="text-[12px] font-medium text-[var(--text-secondary)] uppercase tracking-wide mb-2">Sub-Codec</legend>
      <div class="grid" style="grid-template-columns:repeat(2,1fr)">
        {#each [['aac','AAC (lossy)'],['alac','ALAC (lossless)']] as [v, lbl], i}
          <button onclick={() => options.m4a_subcodec = v} class={seg(options.m4a_subcodec === v, i, 2)}>{lbl}</button>
        {/each}
      </div>
    </fieldset>
    {#if options.m4a_subcodec === 'alac'}
      <fieldset>
        <legend class="text-[12px] font-medium text-[var(--text-secondary)] uppercase tracking-wide mb-2">Bit Depth</legend>
        <div class="grid" style="grid-template-columns:repeat(2,1fr)">
          {#each [[16,'16-bit'],[24,'24-bit']] as [v, lbl], i}
            <button onclick={() => options.bit_depth = v} class={seg(options.bit_depth === v, i, 2)}>{lbl}</button>
          {/each}
        </div>
      </fieldset>
    {/if}

  {:else if options.output_format === 'wma'}
    <fieldset data-tooltip="Standard — stereo lossy · Pro — multi-channel lossy · Lossless — bit-perfect like FLAC but Windows-only">
      <legend class="text-[12px] font-medium text-[var(--text-secondary)] uppercase tracking-wide mb-2">WMA Mode</legend>
      <div class="inline-flex flex-col">
        {#each [['standard','Standard'],['pro','Pro (multi-channel)'],['lossless','Lossless']] as [v, lbl], i}
          <button onclick={() => options.wma_mode = v} class={segV(options.wma_mode === v, i, 3)}>{lbl}</button>
        {/each}
      </div>
    </fieldset>

  {:else if options.output_format === 'alac'}
    <fieldset data-tooltip="16-bit CD-quality · 24-bit studio · 32-bit float for mixing / processing without clipping">
      <legend class="text-[12px] font-medium text-[var(--text-secondary)] uppercase tracking-wide mb-2">Bit Depth</legend>
      <div class="grid" style="grid-template-columns:repeat(3,1fr)">
        {#each [[16,'16-bit'],[24,'24-bit'],[32,'32-bit']] as [v, lbl], i}
          <button onclick={() => options.bit_depth = v} class={seg(options.bit_depth === v, i, 3)}>{lbl}</button>
        {/each}
      </div>
    </fieldset>
    <fieldset data-tooltip="Source — match input · Mono — single channel, smallest · Stereo — two channels · Joint — stereo with shared bits (MP3 only) · 5.1 — surround">
      <legend class="text-[12px] font-medium text-[var(--text-secondary)] uppercase tracking-wide mb-2">Channels</legend>
      <div class="grid" style="grid-template-columns:repeat(3,1fr)">
        {#each [['source','Source'],['mono','Mono'],['stereo','Stereo']] as [v, lbl], i}
          <button onclick={() => options.channels = v} class={seg(options.channels === v, i, 3)}>{lbl}</button>
        {/each}
      </div>
    </fieldset>

  {:else if options.output_format === 'ac3'}
    <fieldset data-tooltip="Source — match input · Mono — single channel, smallest · Stereo — two channels · Joint — stereo with shared bits (MP3 only) · 5.1 — surround">
      <legend class="text-[12px] font-medium text-[var(--text-secondary)] uppercase tracking-wide mb-2">Channels</legend>
      <div class="grid" style="grid-template-columns:repeat(3,1fr)">
        {#each [['mono','Mono'],['stereo','Stereo'],['5.1','5.1']] as [v, lbl], i}
          <button onclick={() => options.channels = v} class={seg(options.channels === v, i, 3)}>{lbl}</button>
        {/each}
      </div>
    </fieldset>
    <fieldset data-tooltip="448 kbps typical for 5.1 broadcast · 48 kHz required">
      <legend class="text-[12px] font-medium text-[var(--text-secondary)] uppercase tracking-wide mb-2">Bitrate — kbps</legend>
      <div class="grid" style="grid-template-columns:repeat(4,1fr)">
        {#each [192, 384, 448, 640] as br, i}
          <button onclick={() => options.ac3_bitrate = br} class={seg(options.ac3_bitrate === br, i, 4)}>{br}</button>
        {/each}
      </div>
    </fieldset>

  {:else if options.output_format === 'dts'}
    <fieldset data-tooltip="Source — match input · Mono — single channel, smallest · Stereo — two channels · Joint — stereo with shared bits (MP3 only) · 5.1 — surround">
      <legend class="text-[12px] font-medium text-[var(--text-secondary)] uppercase tracking-wide mb-2">Channels</legend>
      <div class="grid" style="grid-template-columns:repeat(2,1fr)">
        {#each [['stereo','Stereo'],['5.1','5.1']] as [v, lbl], i}
          <button onclick={() => options.channels = v} class={seg(options.channels === v, i, 2)}>{lbl}</button>
        {/each}
      </div>
    </fieldset>
    <fieldset data-tooltip="FFmpeg encodes DTS core only">
      <legend class="text-[12px] font-medium text-[var(--text-secondary)] uppercase tracking-wide mb-2">Bitrate — kbps</legend>
      <div class="grid" style="grid-template-columns:repeat(2,1fr)">
        {#each [754, 1510] as br, i}
          <button onclick={() => options.dts_bitrate = br} class={seg(options.dts_bitrate === br, i, 2)}>{br}</button>
        {/each}
      </div>
    </fieldset>
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
      <div data-tooltip="Hard peak limiter — prevents samples from exceeding the dBFS ceiling you set. −1.0 dBFS typical for streaming safety.">
        <button onclick={() => options.dsp_limiter_db = options.dsp_limiter_db != null ? null : -1.0}
                class="w-full px-3 py-[5px] rounded-md text-left text-[12px] font-medium border flex items-center gap-2 transition-colors
                       {options.dsp_limiter_db != null ? 'seg-active z-10' : 'seg-inactive border-[var(--border)] text-[color-mix(in_srgb,var(--text-primary)_70%,transparent)] hover:z-10'}">
          <span class="flex-1">Limiter</span>
          {#if options.dsp_limiter_db != null}<span class="font-mono text-[11px]">{options.dsp_limiter_db.toFixed(1)} dBFS</span>{/if}
          <svg class="shrink-0 transition-transform duration-200 {options.dsp_limiter_db != null ? 'rotate-180' : ''}" width="10" height="6" viewBox="0 0 10 6" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"><path d="M1 1l4 4 4-4"/></svg>
        </button>
        {#if options.dsp_limiter_db != null}
          <div class="mt-1 px-1 space-y-1">
            <input type="range" min="-12" max="0" step="0.5" bind:value={options.dsp_limiter_db} class="fade-range"
                   style="--fade-range-pct:{(((options.dsp_limiter_db ?? -1) - -12) / 12) * 100}%" />
            <div class="flex justify-between text-[10px] text-[var(--text-secondary)]"><span>−12 dBFS</span><span>0 dBFS</span></div>
          </div>
        {/if}
      </div>

      <!-- Normalize -->
      <div data-tooltip="EBU R128 loudness normalisation — matches perceived volume to a LUFS target with a true-peak ceiling. Use for batches that need consistent loudness.">
        <button onclick={() => { options.normalize_loudness = !options.normalize_loudness; if (options.normalize_loudness) { options.normalize_lufs ??= -16; options.normalize_true_peak ??= -1; } }}
                class="w-full px-3 py-[5px] rounded-md text-left text-[12px] font-medium border flex items-center gap-2 transition-colors
                       {options.normalize_loudness ? 'seg-active z-10' : 'seg-inactive border-[var(--border)] text-[color-mix(in_srgb,var(--text-primary)_70%,transparent)] hover:z-10'}">
          <span class="flex-1">Normalize</span>
          {#if options.normalize_loudness}
            <span class="font-mono text-[11px]">{options.normalize_lufs ?? -16} LUFS / {options.normalize_true_peak ?? -1} dBTP</span>
          {/if}
          <svg class="shrink-0 transition-transform duration-200 {options.normalize_loudness ? 'rotate-180' : ''}" width="10" height="6" viewBox="0 0 10 6" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"><path d="M1 1l4 4 4-4"/></svg>
        </button>
        {#if options.normalize_loudness}
          <div class="mt-1 px-1 space-y-2">
            <!-- LUFS target presets -->
            <div class="space-y-1"
                 data-tooltip="Integrated loudness target. −23 EBU broadcast · −16 Apple Music / YouTube · −14 Spotify · −9 loud podcast. Lower = quieter.">
              <div class="text-[10px] text-[var(--text-secondary)] px-1">Target LUFS</div>
              <div class="flex">
                {#each [[-23,'Broadcast'],[-16,'Streaming'],[-14,'Streaming+'],[-9,'Podcast']] as [val, label], i}
                  <button onclick={() => options.normalize_lufs = val}
                          data-tooltip={`${val} LUFS — ${label}`}
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
            <div class="space-y-1"
                 data-tooltip="Max allowed inter-sample peak (dBTP). −1.0 safe for lossy codecs · −0.1 mastering ceiling · more negative prevents decoder clipping.">
              <div class="text-[10px] text-[var(--text-secondary)] px-1">True Peak Ceiling</div>
              <input type="range" min="-6" max="-0.1" step="0.1"
                     value={options.normalize_true_peak ?? -1}
                     oninput={(e) => options.normalize_true_peak = parseFloat(e.target.value)}
                     class="fade-range"
                     style="--fade-range-pct:{(((options.normalize_true_peak ?? -1) - -6) / 5.9) * 100}%" />
              <div class="flex justify-between items-center">
                <div class="flex justify-between text-[10px] text-[var(--text-secondary)] flex-1"><span>−6 dBTP</span><span>−0.1 dBTP</span></div>
                <span class="font-mono text-[11px] text-[var(--text-primary)] ml-2">{(options.normalize_true_peak ?? -1).toFixed(1)}</span>
              </div>
            </div>
          </div>
        {/if}
      </div>

      <!-- Highpass -->
      <div data-tooltip="Butterworth highpass filter — cuts everything below the cutoff frequency. 80 Hz removes rumble · 120 Hz for voice.">
        <button onclick={() => options.dsp_highpass_freq = options.dsp_highpass_freq != null ? null : 80}
                class="w-full px-3 py-[5px] rounded-md text-left text-[12px] font-medium border flex items-center gap-2 transition-colors
                       {options.dsp_highpass_freq != null ? 'seg-active z-10' : 'seg-inactive border-[var(--border)] text-[color-mix(in_srgb,var(--text-primary)_70%,transparent)] hover:z-10'}">
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
            <input type="range" min="0" max="100" value={Math.max(0, Math.round(Math.log10(Math.max(1, options.dsp_highpass_freq) / 20) / 3 * 100))} oninput={(e) => options.dsp_highpass_freq = Math.round(20 * Math.pow(1000, parseFloat(e.target.value) / 100))} class="fade-range"
                   style="--fade-range-pct:{Math.max(0, Math.min(100, Math.round(Math.log10(Math.max(1, options.dsp_highpass_freq) / 20) / 3 * 100)))}%" />
            <div class="flex justify-between text-[10px] text-[var(--text-secondary)]"><span>20 Hz</span><span>630 Hz</span><span>20 kHz</span></div>
          </div>
        {/if}
      </div>

      <!-- Lowpass -->
      <div data-tooltip="Butterworth lowpass filter — cuts everything above the cutoff frequency. Tames harsh highs or simulates bandwidth-limited media.">
        <button onclick={() => options.dsp_lowpass_freq = options.dsp_lowpass_freq != null ? null : 8000}
                class="w-full px-3 py-[5px] rounded-md text-left text-[12px] font-medium border flex items-center gap-2 transition-colors
                       {options.dsp_lowpass_freq != null ? 'seg-active z-10' : 'seg-inactive border-[var(--border)] text-[color-mix(in_srgb,var(--text-primary)_70%,transparent)] hover:z-10'}">
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
            <input type="range" min="0" max="100" value={Math.min(100, Math.max(0, Math.round(Math.log10(Math.max(1, options.dsp_lowpass_freq) / 20) / 3 * 100)))} oninput={(e) => options.dsp_lowpass_freq = Math.round(20 * Math.pow(1000, parseFloat(e.target.value) / 100))} class="fade-range"
                   style="--fade-range-pct:{Math.min(100, Math.max(0, Math.round(Math.log10(Math.max(1, options.dsp_lowpass_freq) / 20) / 3 * 100)))}%" />
            <div class="flex justify-between text-[10px] text-[var(--text-secondary)]"><span>20 Hz</span><span>630 Hz</span><span>20 kHz</span></div>
          </div>
        {/if}
      </div>

      <!-- Stereo Width -->
      <div data-tooltip="Mid/side stereo adjustment. −100 collapses to mono · 0 unchanged · +100 maximum width. Useful for widening narrow mixes or mono-compatibility checks.">
        <button onclick={() => options.dsp_stereo_width = options.dsp_stereo_width != null ? null : 0}
                class="w-full px-3 py-[5px] rounded-md text-left text-[12px] font-medium border flex items-center gap-2 transition-colors
                       {options.dsp_stereo_width != null ? 'seg-active z-10' : 'seg-inactive border-[var(--border)] text-[color-mix(in_srgb,var(--text-primary)_70%,transparent)] hover:z-10'}">
          <span class="flex-1">Stereo Width</span>
          {#if options.dsp_stereo_width != null}<span class="font-mono text-[11px]">{options.dsp_stereo_width > 0 ? '+' : ''}{Math.round(options.dsp_stereo_width)}%</span>{/if}
          <svg class="shrink-0 transition-transform duration-200 {options.dsp_stereo_width != null ? 'rotate-180' : ''}" width="10" height="6" viewBox="0 0 10 6" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"><path d="M1 1l4 4 4-4"/></svg>
        </button>
        {#if options.dsp_stereo_width != null}
          <div class="mt-1 px-1 space-y-1">
            <input type="range" min="-100" max="100" step="1" bind:value={options.dsp_stereo_width} class="fade-range"
                   style="--fade-range-pct:{(((options.dsp_stereo_width ?? 0) - -100) / 200) * 100}%" />
            <div class="flex justify-between text-[10px] text-[var(--text-secondary)]"><span>−100 Mono</span><span>0</span><span>+100 Wide</span></div>
          </div>
        {/if}
      </div>

    </div>

  </fieldset>

  <label class="inline-flex items-center gap-2.5 cursor-pointer text-[13px]
                bg-[var(--surface-hint)] border border-[var(--border)] rounded-md px-3 py-2
                {options.preserve_metadata ? 'text-[var(--text-primary)]' : 'text-white/75'}"
         data-tooltip="Keep ID3/Vorbis tags and cover art in the output. Uncheck to strip (removes title, artist, album, embedded artwork).">
    <input type="checkbox" bind:checked={options.preserve_metadata} class="fade-check" />
    Preserve metadata
  </label>

</div>
