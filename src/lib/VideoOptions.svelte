<script>
  import { seg, segV } from './segStyles.js';
  import SilencePad from './SilencePad.svelte';

  let { options = $bindable(), errors = {} } = $props();

  let codecMenuOpen  = $state(false);
  let presetMenuOpen = $state(false);
  let advancedOpen   = $state(false);

  const isSeq = $derived(options.output_format?.startsWith('seq_'));

  // H.264 profile auto-promotion: yuv422p → high422, yuv444p → high444.
  // When pix_fmt forces a profile upgrade, disable baseline/main and annotate high.
  const h264ProfileLocked = $derived(
    options.codec === 'h264' && options.pix_fmt === 'yuv422p' ? 'yuv422p'
    : options.codec === 'h264' && options.pix_fmt === 'yuv444p' ? 'yuv444p'
    : null
  );

  // Force h264_profile to 'high' when baseline/main are no longer reachable.
  $effect(() => {
    if (h264ProfileLocked && (options.h264_profile === 'baseline' || options.h264_profile === 'main')) {
      options.h264_profile = 'high';
    }
  });

  const allCodecs = [
    { value: 'copy', label: 'Copy — stream passthrough' },

    { category: 'Common' },
    { value: 'h264',  label: 'H.264 (AVC)' },
    { value: 'h265',  label: 'H.265 (HEVC)' },
    { value: 'av1',   label: 'AV1' },
    { value: 'vp9',   label: 'VP9' },

    { category: 'Professional' },
    { value: 'prores',   label: 'Apple ProRes' },
    { value: 'dnxhr',    label: 'Avid DNxHR' },
    { value: 'dnxhd',    label: 'Avid DNxHD' },
    { value: 'cineform', label: 'GoPro CineForm' },
    { value: 'hap',      label: 'HAP' },

    { category: 'Broadcast' },
    { value: 'xdcam422', label: 'XDCAM HD422 — 50 Mbps' },
    { value: 'xdcam35',  label: 'XDCAM HD — 35 Mbps' },

    { category: 'Archival' },
    { value: 'ffv1',     label: 'FFV1 (lossless open)' },
    { value: 'rawvideo', label: 'Raw Video (uncompressed)' },
    { value: 'qtrle',    label: 'QuickTime RLE (lossless)' },

    { category: 'Legacy' },
    { value: 'mpeg4',      label: 'MPEG-4 Part 2' },
    { value: 'mpeg2video', label: 'MPEG-2' },
    { value: 'mpeg1video', label: 'MPEG-1' },
    { value: 'mjpeg',      label: 'Motion JPEG' },
    { value: 'dvvideo',    label: 'DV Video' },
    { value: 'theora',     label: 'Theora (OGG)', dev: true },
    { value: 'vp8',        label: 'VP8', dev: true },
    { value: 'wmv2',       label: 'WMV2', dev: true },
  ];

  const visibleCodecs = $derived(
    allCodecs.filter(c => c.category !== undefined || !c.dev || import.meta.env.DEV)
  );

  const selectedCodecLabel = $derived(
    allCodecs.find(c => c.value === options.codec)?.label ?? (options.codec ?? 'Select…')
  );

  const crfQualityLabel = $derived.by(() => {
    const v = options.crf ?? 0;
    if (v < 15) return 'Extreme quality';
    if (v < 20) return 'High quality';
    if (v < 25) return 'Optimized';
    return 'Low quality';
  });

  const PRESETS = ['ultrafast','fast','medium','slow','veryslow']
    .map(p => ({ value: p, label: p }));

  const CODEC_MAX = {
    h264:      { maxW: 4096,  maxH: 2304,  mod: 2 },
    h265:      { maxW: 8192,  maxH: 4320,  mod: 2 },
    vp9:       { maxW: 7680,  maxH: 4320,  mod: 2 },
    av1:       { maxW: 7680,  maxH: 4320,  mod: 2 },
    vp8:       { maxW: 4096,  maxH: 2304,  mod: 2 },
    prores:    { maxW: 8192,  maxH: 4320,  mod: 2 },
    hap:       { maxW: 4096,  maxH: 4096,  mod: 4 },
    dnxhr:     { maxW: 7680,  maxH: 4320,  mod: 4 },
    dnxhd:     { maxW: 1920,  maxH: 1080,  mod: 2 },
    cineform:  { maxW: 4096,  maxH: 2160,  mod: 2 },
    xdcam422:  { maxW: 1920,  maxH: 1080,  mod: 2 },
    xdcam35:   { maxW: 1920,  maxH: 1080,  mod: 2 },
    ffv1:      { maxW: 32768, maxH: 32768, mod: 2 },
    rawvideo:  { maxW: 32768, maxH: 32768, mod: 2 },
    qtrle:     { maxW: 4096,  maxH: 4096,  mod: 2 },
    mpeg4:     { maxW: 4096,  maxH: 2304,  mod: 2 },
    mpeg2video:{ maxW: 1920,  maxH: 1152,  mod: 2 },
    mpeg1video:{ maxW: 4095,  maxH: 4095,  mod: 2 },
    mjpeg:     { maxW: 32768, maxH: 32768, mod: 2 },
    dvvideo:   { maxW: 720,   maxH: 576,   mod: 2 },
    theora:    { maxW: 8192,  maxH: 8192,  mod: 2 },
    wmv2:      { maxW: 2048,  maxH: 2048,  mod: 2 },
  };

  const RESOLUTION_PRESETS = [
    { value: 'original',  label: 'Original',  tooltip: 'Keep source resolution' },
    { value: '854x480',   label: '480p',  tooltip: '854×480 — SD' },
    { value: '1280x720',  label: '720p',  tooltip: '1280×720 — HD' },
    { value: '1920x1080', label: '1080p', tooltip: '1920×1080 — Full HD' },
    { value: '2560x1440', label: '1440p 2K',  tooltip: '2560×1440 — 2K / QHD' },
    { value: '3840x2160', label: '2160p 4K',  tooltip: '3840×2160 — 4K UHD' },
    { value: '6144x3456', label: '6K',    tooltip: '6144×3456 — 6K' },
    { value: '7680x4320', label: '8K',    tooltip: '7680×4320 — 8K UHD' },
  ];

  // ── Resolution state ───────────────────────────────────────────────────────
  let wStr = $state('');
  let hStr = $state('');
  let aspectLocked = $state(false);
  let aspectRatio  = $state(16 / 9);

  // Sync input strings when options.resolution changes externally (preset load, etc.)
  $effect(() => {
    const res = options.resolution;
    if (!res || res === 'original') { wStr = ''; hStr = ''; return; }
    const [w = '', h = ''] = res.split('x');
    if (w !== wStr) wStr = w;
    if (h !== hStr) hStr = h;
  });

  const resolutionError = $derived.by(() => {
    const codec = options.codec;
    const res   = options.resolution;
    if (!res || res === 'original' || !codec) return null;
    const parts = res.split('x');
    if (parts.length !== 2) return null;
    const w = parseInt(parts[0]);
    const h = parseInt(parts[1]);
    if (!w || !h || w < 2 || h < 2) return null;
    const c = CODEC_MAX[codec];
    if (!c) return null;
    if (w > c.maxW || h > c.maxH)
      return `${codec.toUpperCase()} max is ${c.maxW.toLocaleString()}×${c.maxH.toLocaleString()}px`;
    if (c.mod > 1 && (w % c.mod !== 0 || h % c.mod !== 0))
      return `${codec.toUpperCase()} requires dimensions divisible by ${c.mod}`;
    return null;
  });

  function isPresetAllowed(presetValue) {
    if (presetValue === 'original') return true;
    const c = CODEC_MAX[options.codec];
    if (!c) return true;
    const [w, h] = presetValue.split('x').map(Number);
    return w <= c.maxW && h <= c.maxH;
  }

  function selectPreset(value) {
    options.resolution = value;
  }

  function snapEven(n) { return n % 2 === 0 ? n : n + 1; }

  function onWInput(e) {
    wStr = e.target.value;
    const w = parseInt(wStr);
    if (!w || w < 2) return;
    let h = parseInt(hStr);
    if (aspectLocked && h) { h = snapEven(Math.round(w / aspectRatio)); hStr = String(h); }
    if (w && h) options.resolution = `${w}x${h}`;
  }

  function onHInput(e) {
    hStr = e.target.value;
    const h = parseInt(hStr);
    if (!h || h < 2) return;
    let w = parseInt(wStr);
    if (aspectLocked && w) { w = snapEven(Math.round(h * aspectRatio)); wStr = String(w); }
    if (w && h) options.resolution = `${w}x${h}`;
  }

  function toggleAspectLock() {
    aspectLocked = !aspectLocked;
    if (aspectLocked) {
      const w = parseInt(wStr);
      const h = parseInt(hStr);
      if (w && h) aspectRatio = w / h;
    }
  }

  const audioBitrates = [64, 128, 192, 256, 320];
  const sampleRates   = [
    { value: 44100, label: '44.1 kHz — CD' },
    { value: 48000, label: '48 kHz — Video standard' },
    { value: 96000, label: '96 kHz — Hi-Fi' },
  ];
  // ── Trim helpers ───────────────────────────────────────────────────────────
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


</script>

<div class="space-y-3" role="form" aria-label="Video conversion options">

  <!-- ── Codec (dropdown) ──────────────────────────────────────────────── -->
  {#if !isSeq}
  <fieldset data-tooltip="Video encoding codec — H.264 for compatibility, H.265/AV1 for smaller files">
    <legend class="fade-label">
      Video Codec
    </legend>
    <div class="relative">
      <button
        onclick={() => codecMenuOpen = !codecMenuOpen}
        class="w-full flex items-center justify-between px-3 py-[5px] rounded-md border
               border-[var(--border)] seg-inactive text-[var(--text-primary)] text-[13px]
               transition-colors"
      >
        <span class="text-[12px]">{selectedCodecLabel}</span>
        <svg class="w-3.5 h-3.5 text-[var(--text-secondary)] shrink-0 transition-transform
                    {codecMenuOpen ? 'rotate-180' : ''}"
             viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5">
          <path d="M4 6l4 4 4-4"/>
        </svg>
      </button>
      {#if codecMenuOpen}
        <!-- svelte-ignore a11y_no_static_element_interactions -->
        <div class="fixed inset-0 z-40" onmousedown={() => codecMenuOpen = false}></div>
        <div class="absolute left-0 right-0 top-full mt-1 z-50
                    bg-[var(--surface-panel)] border border-[var(--border)] rounded-lg shadow-xl py-1 animate-fade-in">
          {#each visibleCodecs as item}
            {#if item.category}
              <div class="px-3 pt-2 pb-0.5 text-[10px] font-medium uppercase tracking-wider
                          text-[var(--text-secondary)] select-none">{item.category}</div>
            {:else}
              <button
                onmousedown={(e) => { e.stopPropagation(); options.codec = item.value; codecMenuOpen = false; }}
                class="w-full text-left px-3 py-[5px] text-[13px] transition-colors cursor-default outline-none
                       hover:bg-[var(--surface-raised)] hover:text-[var(--text-primary)]
                       text-[color-mix(in_srgb,var(--text-primary)_80%,transparent)]"
              >{item.label}</button>
            {/if}
          {/each}
        </div>
      {/if}
    </div>
  </fieldset>
  {/if}

  <!-- ── Resolution ────────────────────────────────────────────────────── -->
  {#if (options.codec !== 'copy' && options.codec !== 'dvvideo' && options.output_format !== 'gif') || isSeq}
  <fieldset data-tooltip="Output resolution — choose a preset or type custom W×H. Dimmed presets exceed this codec's limit.">
    <legend class="fade-label">Resolution</legend>

    {#snippet resPill(preset)}
      {@const allowed = isPresetAllowed(preset.value)}
      {@const active  = options.resolution === preset.value}
      <button
        onclick={() => allowed && selectPreset(preset.value)}
        data-tooltip={allowed ? preset.tooltip : `Exceeds ${(options.codec ?? '').toUpperCase()} max resolution`}
        class="px-2.5 py-[5px] rounded text-[11px] font-medium border transition-colors
               {active
                 ? 'bg-[color-mix(in_srgb,var(--accent)_37.5%,#000)] text-white border-[var(--border)]'
                 : allowed
                   ? 'seg-inactive border-[var(--border)] text-[color-mix(in_srgb,var(--text-primary)_70%,transparent)] hover:text-[var(--text-primary)]'
                   : 'border-[var(--border)] text-[var(--text-secondary)]/25 cursor-not-allowed'}"
      >{preset.label}</button>
    {/snippet}
    <div class="flex gap-1 mb-1">
      {#each RESOLUTION_PRESETS.slice(0, 4) as preset}{@render resPill(preset)}{/each}
    </div>
    <div class="flex gap-1 mb-2.5">
      {#each RESOLUTION_PRESETS.slice(4) as preset}{@render resPill(preset)}{/each}
    </div>

    <div class="flex items-center gap-1.5">
      <input
        type="number" min="2" max="16384" step="2"
        value={wStr}
        oninput={onWInput}
        placeholder="W"
        disabled={options.resolution === 'original'}
        class="flex-1 min-w-0 px-2 py-1.5 rounded-md border text-[12px] text-center font-mono
               bg-[var(--surface)] placeholder:text-[var(--text-muted)] focus:outline-none
               disabled:opacity-30 disabled:cursor-not-allowed
               {resolutionError
                 ? 'border-red-500 text-red-400 focus:border-red-400'
                 : 'border-[var(--border)] text-[var(--text-primary)] focus:border-[var(--accent)]'}"
      />
      <button
        onclick={toggleAspectLock}
        data-tooltip={aspectLocked ? 'Aspect ratio locked — click to unlock' : 'Click to lock aspect ratio'}
        class="shrink-0 w-7 h-7 flex items-center justify-center rounded border transition-colors
               {aspectLocked
                 ? 'border-[var(--accent)] text-[var(--accent)] bg-[color-mix(in_srgb,var(--accent)_15%,transparent)]'
                 : 'border-[var(--border)] text-[var(--text-secondary)] hover:border-[var(--text-secondary)]'}"
      >
        {#if aspectLocked}
          <svg width="11" height="11" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round">
            <rect x="3" y="7" width="10" height="8" rx="1.5"/>
            <path d="M5.5 7V5a2.5 2.5 0 0 1 5 0v2"/>
          </svg>
        {:else}
          <svg width="11" height="11" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round">
            <rect x="3" y="7" width="10" height="8" rx="1.5"/>
            <path d="M5.5 7V5a2.5 2.5 0 0 1 5 0"/>
          </svg>
        {/if}
      </button>
      <input
        type="number" min="2" max="16384" step="2"
        value={hStr}
        oninput={onHInput}
        placeholder="H"
        disabled={options.resolution === 'original'}
        class="flex-1 min-w-0 px-2 py-1.5 rounded-md border text-[12px] text-center font-mono
               bg-[var(--surface)] placeholder:text-[var(--text-muted)] focus:outline-none
               disabled:opacity-30 disabled:cursor-not-allowed
               {resolutionError
                 ? 'border-red-500 text-red-400 focus:border-red-400'
                 : 'border-[var(--border)] text-[var(--text-primary)] focus:border-[var(--accent)]'}"
      />
    </div>

    {#if resolutionError}
      <p class="text-[11px] text-red-400 mt-1.5">{resolutionError}</p>
    {:else if errors.resolution}
      <p class="text-[11px] text-red-500 mt-1.5">{errors.resolution}</p>
    {/if}
  </fieldset>
  {/if}

  <!-- ── Quality slider ───────────────────────────────────────────────── -->
  {#if (options.output_format !== 'gif' && options.codec !== 'copy' && ['h264','h265','vp9','av1'].includes(options.codec) && (options.video_bitrate_mode ?? 'crf') === 'crf') || options.output_format === 'seq_jpg'}
    <fieldset data-tooltip="Drag right for better quality (lower CRF), left for smaller file (higher CRF)">
      <legend class="fade-label">Quality — {crfQualityLabel} · {options.crf ?? 23}</legend>
      <input type="range" min="0" max="51" step="1"
             value={51 - (options.crf ?? 23)}
             oninput={(e) => options.crf = 51 - Number(e.target.value)}
             class="fade-range"
             style="--fade-range-pct:{((51 - (options.crf ?? 23)) / 51) * 100}%" />
      <div class="flex justify-between text-[10px] text-[var(--text-secondary)] mt-1">
        <span>Worst</span>
        <span>Lossless</span>
      </div>
    </fieldset>
  {/if}

  <!-- ── Frame rate (exposed for image sequences) ─────────────────────── -->
  {#if isSeq}
    <fieldset data-tooltip="Output frame rate — controls how many frames are extracted per second. Original keeps the source rate.">
      <legend class="fade-label">Frame Rate</legend>
      <div class="grid" style="grid-template-columns:repeat(5,1fr)">
        {#each ['original','24','25','30','60'] as r, i}
          <button onclick={() => options.frame_rate = r} class={seg(options.frame_rate === r, i, 5)}>{r === 'original' ? 'Orig' : r}</button>
        {/each}
      </div>
    </fieldset>
  {/if}

  <!-- ── ProRes profile ───────────────────────────────────────────────── -->
  {#if options.codec === 'prores'}
    <fieldset data-tooltip="Proxy — offline edit · LT — lightweight · 422 — standard · HQ — high quality · 4444 — full chroma/alpha · 4444XQ — extreme quality">
      <legend class="fade-label">ProRes Profile</legend>
      <div class="inline-flex flex-col">
        {#each [[0,'Proxy — 422 LT'],[1,'LT — 422 LT'],[2,'422 — Standard'],[3,'HQ — High Quality'],[4,'4444'],[5,'4444 XQ']] as [v, lbl], i}
          <button onclick={() => options.prores_profile = v} class={segV((options.prores_profile ?? 3) === v, i, 6)}>{lbl}</button>
        {/each}
      </div>
    </fieldset>
  {/if}

  <!-- ── Advanced ──────────────────────────────────────────────────────── -->
  <div class="border border-[var(--border)] rounded-md overflow-hidden">
    <button
      onclick={() => advancedOpen = !advancedOpen}
      class="w-full flex items-center justify-between px-3 py-2 text-[12px]
             text-[var(--text-secondary)] hover:text-[var(--text-primary)]
             bg-[var(--surface-hint)] transition-colors"
    >
      <span>Advanced</span>
      <svg class="w-3.5 h-3.5 shrink-0 transition-transform {advancedOpen ? 'rotate-180' : ''}"
           viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5">
        <path d="M4 6l4 4 4-4"/>
      </svg>
    </button>

    {#if advancedOpen}
      <div class="space-y-3 p-3 pt-2">

        <!-- ── Quality mode ──────────────────────────────────────────── -->
        {#if options.output_format !== 'gif' && options.codec !== 'copy' && ['h264','h265','vp9','av1'].includes(options.codec)}
          <fieldset data-tooltip="CRF — quality-based, variable size · VBR — target bitrate, variable quality · CBR — fixed bitrate for streaming">
            <legend class="fade-label">Quality Mode</legend>
            <div class="grid" style="grid-template-columns:repeat(3,1fr)">
              {#each [['crf','CRF'],['vbr','VBR'],['cbr','CBR']] as [v, lbl], i}
                <button onclick={() => options.video_bitrate_mode = v}
                        class={seg((options.video_bitrate_mode ?? 'crf') === v, i, 3)}>{lbl}</button>
              {/each}
            </div>
          </fieldset>

          {#if (options.video_bitrate_mode ?? 'crf') !== 'crf'}
            <fieldset data-tooltip="Target video bitrate in kbps. VBR varies around this; CBR holds it fixed.">
              <legend class="fade-label">Video Bitrate — kbps</legend>
              <div class="grid" style="grid-template-columns:repeat(5,1fr)">
                {#each [1000, 2000, 4000, 8000, 16000] as b, i}
                  <button onclick={() => options.video_bitrate = b}
                          class={seg((options.video_bitrate ?? 4000) === b, i, 5)}>{b >= 1000 ? b/1000 + 'M' : b}</button>
                {/each}
              </div>
            </fieldset>
          {/if}
        {/if}

        <!-- ── Audio track ────────────────────────────────────────────── -->
        <fieldset data-tooltip="Strip the audio track from the video output">
          <legend class="fade-label">Audio Track</legend>
          <label class="inline-flex items-center gap-2.5 cursor-pointer text-[13px]
                        bg-[var(--surface-hint)] border border-[var(--border)] rounded-md px-3 py-2
                        {options.remove_audio ? 'text-[var(--text-primary)]' : 'text-white/75'}">
            <input type="checkbox"
                   checked={options.remove_audio === true}
                   onchange={(e) => { options.remove_audio = e.currentTarget.checked; options.extract_audio = false; }}
                   class="fade-check" />
            Remove audio
          </label>
        </fieldset>

        <!-- ── Trim ──────────────────────────────────────────────────── -->
        <fieldset data-tooltip="Trim the output — enter time as MM:SS or raw seconds.">
          <legend class="fade-label">Trim</legend>
          <div class="flex gap-3 items-center">
            <div class="flex-1">
              <input id="vid-trim-start" type="text" placeholder="Start"
                value={trimStartRaw} oninput={onTrimStartInput}
                class="w-full px-3 py-1.5 rounded-md border border-[var(--border)]
                       bg-[var(--surface)] text-[var(--text-primary)] text-[13px]
                       placeholder:text-[var(--text-muted)]
                       focus:outline-none focus:border-[var(--accent)]"
              />
            </div>
            <div class="flex-1">
              <input id="vid-trim-end" type="text" placeholder="End"
                value={trimEndRaw} oninput={onTrimEndInput}
                class="w-full px-3 py-1.5 rounded-md text-[13px]
                       focus:outline-none focus:border-[var(--accent)]
                       bg-[var(--surface)] text-[var(--text-primary)]
                       placeholder:text-[var(--text-muted)]
                       {errors.video_trim ? 'border border-red-500' : 'border border-[var(--border)]'}"
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
          {#if errors.video_trim}
            <p class="text-[11px] text-red-500 mt-1">{errors.video_trim}</p>
          {/if}
        </fieldset>

        {#if options.output_format !== 'gif' && options.codec !== 'copy'}
          <!-- ── Encode Preset ────────────────────────────────────────── -->
          {#if ['h264','h265'].includes(options.codec)}
            <fieldset data-tooltip="Slower preset = better compression at same quality">
              <legend class="fade-label">Encode Preset</legend>
              <div class="relative">
                <button
                  onclick={() => presetMenuOpen = !presetMenuOpen}
                  class="w-full flex items-center justify-between px-3 py-[5px] rounded-md border
                         border-[var(--border)] seg-inactive text-[var(--text-primary)] text-[13px]
                         transition-colors"
                >
                  <span class="text-[12px]">{options.preset ?? 'medium'}</span>
                  <svg class="w-3.5 h-3.5 text-[var(--text-secondary)] shrink-0 transition-transform
                              {presetMenuOpen ? 'rotate-180' : ''}"
                       viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5">
                    <path d="M4 6l4 4 4-4"/>
                  </svg>
                </button>
                {#if presetMenuOpen}
                  <!-- svelte-ignore a11y_no_static_element_interactions -->
                  <div class="fixed inset-0 z-40" onmousedown={() => presetMenuOpen = false}></div>
                  <div class="absolute left-0 right-0 top-full mt-1 z-50
                              bg-[var(--surface-panel)] border border-[var(--border)] rounded-lg shadow-xl py-1 animate-fade-in">
                    {#each PRESETS as item}
                      <button
                        onmousedown={(e) => { e.stopPropagation(); options.preset = item.value; presetMenuOpen = false; }}
                        class="w-full text-left px-3 py-[5px] text-[13px] transition-colors cursor-default outline-none
                               hover:bg-[var(--surface-raised)] hover:text-[var(--text-primary)]
                               text-[color-mix(in_srgb,var(--text-primary)_80%,transparent)]"
                      >{item.label}</button>
                    {/each}
                  </div>
                {/if}
              </div>
            </fieldset>

            <fieldset data-tooltip="baseline — max compatibility · main — consumer · high — streaming / archival · yuv422p/yuv444p forces High 4:2:2 or High 4:4:4">
              <legend class="fade-label">Profile</legend>
              <div class="grid" style="grid-template-columns:repeat(3,1fr)">
                {#each ['baseline','main','high'] as p, i}
                  {@const locked = h264ProfileLocked !== null && (p === 'baseline' || p === 'main')}
                  <button
                    onclick={() => !locked && (options.h264_profile = p)}
                    disabled={locked}
                    class="{seg(options.h264_profile === p, i, 3)} {locked ? 'opacity-50 cursor-not-allowed pointer-events-none' : ''}"
                  >
                    {#if p === 'high' && h264ProfileLocked === 'yuv422p'}
                      high<span class="text-xs opacity-60"> (→ High 4:2:2)</span>
                    {:else if p === 'high' && h264ProfileLocked === 'yuv444p'}
                      high<span class="text-xs opacity-60"> (→ High 4:4:4)</span>
                    {:else}
                      {p}
                    {/if}
                  </button>
                {/each}
              </div>
            </fieldset>

            <fieldset data-tooltip="none for general use · film / animation / grain / stillimage optimize for content type">
              <legend class="fade-label">Tune</legend>
              <div class="grid" style="grid-template-columns:repeat(4,1fr)">
                {#each ['none','film','animation','grain'] as t, i}
                  <button onclick={() => options.tune = t} class={seg(options.tune === t, i, 4)}>{t}</button>
                {/each}
              </div>
            </fieldset>
          {/if}

          {#if options.codec === 'vp9'}
            <fieldset data-tooltip="VP9 speed/deadline: 0 best quality · 5 fastest">
              <legend class="fade-label">Speed — {options.vp9_speed}</legend>
              <input type="range" min="0" max="5" step="1" bind:value={options.vp9_speed} class="fade-range"
                     style="--fade-range-pct:{((options.vp9_speed ?? 0) / 5) * 100}%" />
              <div class="flex justify-between text-[10px] text-[var(--text-secondary)] mt-1"><span>0 best</span><span>5 fastest</span></div>
            </fieldset>
          {/if}

          {#if options.codec === 'av1'}
            <fieldset data-tooltip="AV1 cpu-used: 0 slowest / best · 10 fastest / worst">
              <legend class="fade-label">Speed — {options.av1_speed}</legend>
              <input type="range" min="0" max="10" step="1" bind:value={options.av1_speed} class="fade-range"
                     style="--fade-range-pct:{((options.av1_speed ?? 0) / 10) * 100}%" />
              <div class="flex justify-between text-[10px] text-[var(--text-secondary)] mt-1"><span>0 best</span><span>10 fastest</span></div>
            </fieldset>
          {/if}

          <fieldset data-tooltip="yuv420p universal · yuv422p broadcast · yuv444p best quality (not always supported)">
            <legend class="fade-label">Pixel Format</legend>
            <div class="grid" style="grid-template-columns:repeat(3,1fr)">
              {#each ['yuv420p','yuv422p','yuv444p'] as p, i}
                <button onclick={() => options.pix_fmt = p} class={seg(options.pix_fmt === p, i, 3)}>{p}</button>
              {/each}
            </div>
          </fieldset>
        {/if}

        {#if options.output_format !== 'gif'}
          {#if options.codec === 'hap'}
            <fieldset data-tooltip="HAP — DXT1 no alpha · HAP Alpha — DXT5 with alpha · HAP Q — YCoCg better quality · HAP Q Alpha — YCoCg with alpha">
              <legend class="fade-label">HAP Variant</legend>
              <div class="grid" style="grid-template-columns:repeat(4,1fr)">
                {#each [['hap','HAP'],['hap_alpha','HAP α'],['hap_q','HAP Q'],['hap_q_alpha','HAP Qα']] as [v, lbl], i}
                  <button onclick={() => options.hap_format = v} class={seg((options.hap_format ?? 'hap') === v, i, 4)}>{lbl}</button>
                {/each}
              </div>
            </fieldset>
          {/if}

          {#if options.codec === 'dnxhr'}
            <fieldset data-tooltip="LB low bandwidth · SQ standard · HQ high quality · HQX 12-bit · 444 full 4:4:4 chroma">
              <legend class="fade-label">Profile</legend>
              <div class="inline-flex flex-col">
                {#each [['dnxhr_lb','LB — Low Bandwidth'],['dnxhr_sq','SQ — Standard Quality'],['dnxhr_hq','HQ — High Quality'],['dnxhr_hqx','HQX — High Quality 12-bit'],['dnxhr_444','444 — 4:4:4']] as [v, lbl], i}
                  <button onclick={() => options.dnxhr_profile = v} class={segV((options.dnxhr_profile ?? 'dnxhr_sq') === v, i, 5)}>{lbl}</button>
                {/each}
              </div>
            </fieldset>
          {/if}

          {#if options.codec === 'dnxhd'}
            <fieldset data-tooltip="Bitrate must pair with source resolution and frame rate — e.g. 185M for 1080p/29.97, 175M for 1080p/23.976">
              <legend class="fade-label">Bitrate</legend>
              <div class="grid" style="grid-template-columns:repeat(4,1fr)">
                {#each [[36,'36M'],[120,'120M'],[175,'175M'],[185,'185M']] as [v, lbl], i}
                  <button onclick={() => options.dnxhd_bitrate = v} class={seg((options.dnxhd_bitrate ?? 185) === v, i, 4)}>{lbl}</button>
                {/each}
              </div>
              <p class="text-[11px] text-[var(--text-secondary)] mt-1.5">Mismatched resolution × fps will error. Use DNxHR for resolution-independent encoding.</p>
            </fieldset>
          {/if}

          {#if options.codec === 'dvvideo'}
            <fieldset data-tooltip="NTSC: 720×480 at 29.97 fps · PAL: 720×576 at 25 fps — DV forces these exact specs">
              <legend class="fade-label">Standard</legend>
              <div class="grid" style="grid-template-columns:repeat(2,1fr)">
                {#each [['ntsc','NTSC (720×480)'],['pal','PAL (720×576)']] as [v, lbl], i}
                  <button onclick={() => options.dv_standard = v} class={seg((options.dv_standard ?? 'ntsc') === v, i, 2)}>{lbl}</button>
                {/each}
              </div>
            </fieldset>
          {/if}

          <fieldset data-tooltip="Output frame rate — 24 film · 25 PAL · 30 NTSC · 60 smooth motion / gaming · Orig keeps source">
            <legend class="fade-label">Frame Rate</legend>
            <div class="grid" style="grid-template-columns:repeat(5,1fr)">
              {#each ['original','24','25','30','60'] as r, i}
                <button onclick={() => options.frame_rate = r} class={seg(options.frame_rate === r, i, 5)}>{r === 'original' ? 'Orig' : r}</button>
              {/each}
            </div>
          </fieldset>
        {/if}

        {#if options.output_format === 'webm'}
          <fieldset data-tooltip="CRF — quality-based, variable size · CBR — fixed bitrate for streaming · Constrained VBR — VBR capped to a target">
            <legend class="fade-label">Bitrate Mode</legend>
            <div class="grid" style="grid-template-columns:repeat(3,1fr)">
              {#each [['crf','CRF'],['cbr','CBR'],['cvbr','Constrained VBR']] as [v, lbl], i}
                <button onclick={() => options.webm_bitrate_mode = v} class={seg(options.webm_bitrate_mode === v, i, 3)}>{lbl}</button>
              {/each}
            </div>
          </fieldset>
          {#if options.webm_bitrate_mode === 'cbr' || options.webm_bitrate_mode === 'cvbr'}
            <fieldset data-tooltip="Target video bitrate for CBR (fixed) or CVBR (capped). Independent of audio bitrate.">
              <legend class="fade-label">Video Bitrate — kbps</legend>
              <div class="grid" style="grid-template-columns:repeat(4,1fr)">
                {#each [1000, 2000, 4000, 8000] as b, i}
                  <button onclick={() => options.webm_video_bitrate = b} class={seg(options.webm_video_bitrate === b, i, 4)}>{b}</button>
                {/each}
              </div>
            </fieldset>
          {/if}
        {/if}

        {#if options.output_format === 'mkv'}
          <fieldset data-tooltip="None — discard subtitles · Copy — keep as selectable track · Burn-in — render subtitles permanently into the picture">
            <legend class="fade-label">Subtitle Track</legend>
            <div class="grid" style="grid-template-columns:repeat(3,1fr)">
              {#each [['none','None'],['copy','Copy'],['burn','Burn-in']] as [v, lbl], i}
                <button onclick={() => options.mkv_subtitle = v} class={seg(options.mkv_subtitle === v, i, 3)}>{lbl}</button>
              {/each}
            </div>
          </fieldset>
        {/if}

        {#if options.output_format === 'avi'}
          <fieldset data-tooltip="Legacy format — no H.265 or modern codec support">
            <legend class="fade-label">Video Bitrate — kbps</legend>
            <div class="grid" style="grid-template-columns:repeat(4,1fr)">
              {#each [1000, 4000, 8000, 20000] as b, i}
                <button onclick={() => options.avi_video_bitrate = b} class={seg(options.avi_video_bitrate === b, i, 4)}>{b}</button>
              {/each}
            </div>
          </fieldset>
        {/if}

        {#if options.output_format === 'gif'}
          <fieldset data-tooltip="GIF output width in pixels — height auto-scaled to preserve aspect ratio">
            <legend class="fade-label">Output Width (px)</legend>
            <div class="grid" style="grid-template-columns:repeat(4,1fr)">
              {#each ['320', '480', '640', 'original'] as w, i}
                <button onclick={() => options.gif_width = w} class={seg(options.gif_width === w, i, 4)}>{w}</button>
              {/each}
            </div>
          </fieldset>
          <fieldset data-tooltip="GIF frame rate — lower = smaller file. 10 fps typical for memes · 15 fps smoother · Orig keeps source rate">
            <legend class="fade-label">Frame Rate</legend>
            <div class="grid" style="grid-template-columns:repeat(4,1fr)">
              {#each ['5', '10', '15', 'original'] as r, i}
                <button onclick={() => options.gif_fps = r} class={seg(options.gif_fps === r, i, 4)}>{r === 'original' ? 'Orig' : r + ' fps'}</button>
              {/each}
            </div>
          </fieldset>
          <fieldset data-tooltip="Infinite — loop forever · Once — play through then stop · No loop — single play in viewers that honor the flag">
            <legend class="fade-label">Loop</legend>
            <div class="grid" style="grid-template-columns:repeat(3,1fr)">
              {#each [['infinite','Infinite'],['once','Once'],['none','No loop']] as [v, lbl], i}
                <button onclick={() => options.gif_loop = v} class={seg(options.gif_loop === v, i, 3)}>{lbl}</button>
              {/each}
            </div>
          </fieldset>
          <fieldset data-tooltip="Max colors in the shared palette. 32/64 smaller file · 256 best color fidelity but largest">
            <legend class="fade-label">Palette Size</legend>
            <div class="grid" style="grid-template-columns:repeat(4,1fr)">
              {#each [32, 64, 128, 256] as p, i}
                <button onclick={() => options.gif_palette_size = p} class={seg(options.gif_palette_size === p, i, 4)}>{p}</button>
              {/each}
            </div>
          </fieldset>
          <fieldset data-tooltip="None — flat banding · Bayer — ordered dither, retro look · Floyd-Steinberg — error diffusion, smoothest but busy">
            <legend class="fade-label">Dither</legend>
            <div class="grid" style="grid-template-columns:repeat(3,1fr)">
              {#each [['none','None'],['bayer','Bayer'],['floyd','Floyd-Steinberg']] as [v, lbl], i}
                <button onclick={() => options.gif_dither = v} class={seg(options.gif_dither === v, i, 3)}>{lbl}</button>
              {/each}
            </div>
          </fieldset>
        {/if}

        <!-- ── Audio bitrate / sample rate ───────────────────────────── -->
        {#if !options.remove_audio}
          <fieldset data-tooltip="Audio track bitrate — 128 standard · 192 music · 256–320 near-transparent. Independent from video bitrate.">
            <legend class="fade-label">Audio Bitrate</legend>
            <div class="grid" style="grid-template-columns: repeat({audioBitrates.length}, 1fr)">
              {#each audioBitrates as br, i}
                <button onclick={() => options.bitrate = br}
                        class={seg(options.bitrate === br, i, audioBitrates.length)}
                >{br}</button>
              {/each}
            </div>
            <p class="text-[11px] text-[var(--text-secondary)] mt-1">kbps</p>
          </fieldset>

          <fieldset data-tooltip="Audio sample rate — 48 kHz standard for video · 44.1 kHz CD source · 96 kHz for high-end masters">
            <legend class="fade-label">Sample Rate</legend>
            <div class="inline-flex flex-col">
              {#each sampleRates as sr, i}
                <button onclick={() => options.sample_rate = sr.value}
                        class={segV(options.sample_rate === sr.value, i, sampleRates.length)}>
                  {sr.label}
                </button>
              {/each}
            </div>
          </fieldset>
        {/if}

        <!-- ── Silence / Black-frame padding ────────────────────────── -->
        {#if options.output_format !== 'gif'}
          <fieldset data-tooltip="Prepend/append silence + black frames to the output. Front pad adds black frames before the video starts; end pad adds them after it ends.">
            <legend class="fade-label">Silence Padding</legend>
            <SilencePad bind:padFront={options.pad_front} bind:padEnd={options.pad_end} />
          </fieldset>
        {/if}

        <!-- ── Preserve metadata ──────────────────────────────────────── -->
        <label class="inline-flex items-center gap-2.5 cursor-pointer text-[13px]
                      bg-[var(--surface-hint)] border border-[var(--border)] rounded-md px-3 py-2
                      {options.preserve_metadata ? 'text-[var(--text-primary)]' : 'text-white/75'}"
               data-tooltip="Keep title, encoder, GPS, and other container tags in the output. Uncheck to strip (removes location from phone-recorded video).">
          <input type="checkbox" bind:checked={options.preserve_metadata} class="fade-check" />
          Preserve metadata
        </label>

      </div>
    {/if}
  </div>

</div>
