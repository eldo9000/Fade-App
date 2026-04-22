<script>
  import { seg, segV } from './segStyles.js';

  let { options = $bindable(), errors = {} } = $props();

  let codecMenuOpen  = $state(false);
  let presetMenuOpen = $state(false);

  const allCodecs = [
    { value: 'copy',     label: 'Copy — stream passthrough' },
    null,
    { value: 'h264',     label: 'H.264 (AVC)' },
    { value: 'h265',     label: 'H.265 (HEVC)' },
    { value: 'h266',     label: 'H.266 (VVC)', dev: true },
    null,
    { value: 'vp9',      label: 'VP9' },
    { value: 'av1',      label: 'AV1' },
    { value: 'vp8',      label: 'VP8', dev: true },
    null,
    { value: 'hap',      label: 'HAP' },
    { value: 'dnxhr',    label: 'Avid DNxHR' },
    { value: 'dnxhd',    label: 'Avid DNxHD' },
    { value: 'dvvideo',  label: 'DV Video' },
  ];

  const visibleCodecs = $derived(
    allCodecs.filter(c => c === null || !c.dev || import.meta.env.DEV)
  );

  const selectedCodecLabel = $derived(
    allCodecs.find(c => c && c.value === options.codec)?.label ?? (options.codec ?? 'Select…')
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

  const resolutions = [
    { value: 'original',  label: 'Original' },
    { value: '1920x1080', label: '1080p' },
    { value: '1280x720',  label: '720p' },
    { value: '854x480',   label: '480p' },
  ];
  const audioBitrates = [64, 128, 192, 256, 320];
  const sampleRates   = [
    { value: 44100, label: '44.1 kHz — CD' },
    { value: 48000, label: '48 kHz — Video standard' },
    { value: 96000, label: '96 kHz — Hi-Fi' },
  ];
  const audioFormats = ['mp3','wav','flac','aac','opus'];

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

</script>

<div class="space-y-3" role="form" aria-label="Video conversion options">

  <!-- ── Codec (dropdown) ──────────────────────────────────────────────── -->
  <fieldset data-tooltip="Video encoding codec — H.264 for compatibility, H.265/AV1 for smaller files">
    <legend class="text-[12px] font-medium text-[var(--text-secondary)] uppercase tracking-wide mb-1.5">
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
            {#if item === null}
              <div class="my-1 border-t border-[var(--border)]" role="separator"></div>
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

  <!-- ── Resolution (segmented) ────────────────────────────────────────── -->
  <fieldset data-tooltip="Scale the output video resolution">
    <legend class="text-[12px] font-medium text-[var(--text-secondary)] uppercase tracking-wide mb-1.5">
      Resolution
    </legend>
    <div class="grid" style="grid-template-columns: repeat({resolutions.length}, 1fr)">
      {#each resolutions as r, i}
        <button onclick={() => options.resolution = r.value}
                class={seg(options.resolution === r.value, i, resolutions.length)}
        >{r.label}</button>
      {/each}
    </div>
    {#if errors.resolution}
      <p class="text-[11px] text-red-500 mt-1.5">{errors.resolution}</p>
    {/if}
  </fieldset>

  <!-- ── Audio track ────────────────────────────────────────────────────── -->
  <fieldset data-tooltip="Strip the audio track from the video output">
    <legend class="text-[12px] font-medium text-[var(--text-secondary)] uppercase tracking-wide mb-1.5">
      Audio Track
    </legend>
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

  <!-- ── Trim ───────────────────────────────────────────────────────────── -->
  <fieldset data-tooltip="Trim the output — enter time as MM:SS or raw seconds.">
    <legend class="text-[12px] font-medium text-[var(--text-secondary)] uppercase tracking-wide mb-1.5">
      Trim
    </legend>
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

  <!-- ── Format-specific controls ──────────────────────────────────────── -->

  {#if options.output_format !== 'gif'}
    <!-- Quality / CRF (applies to re-encoded codecs, not copy) -->
    {#if options.codec !== 'copy'}
      {#if ['h264','h265','vp9','av1'].includes(options.codec)}
        <fieldset data-tooltip="0 lossless · 18–28 typical · 51 worst · lower = better quality">
          <legend class="text-[12px] font-medium text-[var(--text-secondary)] uppercase tracking-wide mb-1.5">Quality — CRF {options.crf}</legend>
          <input type="range" min="0" max="51" step="1" bind:value={options.crf} class="fade-range"
                 style="--fade-range-pct:{((options.crf ?? 0) / 51) * 100}%" />
          <div class="grid grid-cols-3 text-[10px] text-[var(--text-secondary)] mt-1">
            <span class="text-left">0 lossless</span>
            <span class="text-center">{crfQualityLabel}</span>
            <span class="text-right">51 worst</span>
          </div>
        </fieldset>
      {/if}

      {#if ['h264','h265'].includes(options.codec)}
        <fieldset data-tooltip="Slower preset = better compression at same quality">
          <legend class="text-[12px] font-medium text-[var(--text-secondary)] uppercase tracking-wide mb-1.5">Encode Preset</legend>
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

      {/if}

      {#if options.codec === 'h264' || options.codec === 'h265'}
        <fieldset data-tooltip="baseline — max compatibility · main — consumer · high — streaming / archival">
          <legend class="text-[12px] font-medium text-[var(--text-secondary)] uppercase tracking-wide mb-1.5">Profile</legend>
          <div class="grid" style="grid-template-columns:repeat(3,1fr)">
            {#each ['baseline','main','high'] as p, i}
              <button onclick={() => options.h264_profile = p} class={seg(options.h264_profile === p, i, 3)}>{p}</button>
            {/each}
          </div>
        </fieldset>
        <fieldset data-tooltip="none for general use · film / animation / grain / stillimage optimize for content type">
          <legend class="text-[12px] font-medium text-[var(--text-secondary)] uppercase tracking-wide mb-1.5">Tune</legend>
          <div class="grid" style="grid-template-columns:repeat(4,1fr)">
            {#each ['none','film','animation','grain'] as t, i}
              <button onclick={() => options.tune = t} class={seg(options.tune === t, i, 4)}>{t}</button>
            {/each}
          </div>
        </fieldset>
      {/if}

      {#if options.codec === 'vp9'}
        <fieldset data-tooltip="VP9 speed/deadline: 0 best quality · 5 fastest">
          <legend class="text-[12px] font-medium text-[var(--text-secondary)] uppercase tracking-wide mb-1.5">Speed — {options.vp9_speed}</legend>
          <input type="range" min="0" max="5" step="1" bind:value={options.vp9_speed} class="fade-range"
                 style="--fade-range-pct:{((options.vp9_speed ?? 0) / 5) * 100}%" />
          <div class="flex justify-between text-[10px] text-[var(--text-secondary)] mt-1"><span>0 best</span><span>5 fastest</span></div>
        </fieldset>
      {/if}

      {#if options.codec === 'av1'}
        <fieldset data-tooltip="AV1 cpu-used: 0 slowest / best · 10 fastest / worst">
          <legend class="text-[12px] font-medium text-[var(--text-secondary)] uppercase tracking-wide mb-1.5">Speed — {options.av1_speed}</legend>
          <input type="range" min="0" max="10" step="1" bind:value={options.av1_speed} class="fade-range"
                 style="--fade-range-pct:{((options.av1_speed ?? 0) / 10) * 100}%" />
          <div class="flex justify-between text-[10px] text-[var(--text-secondary)] mt-1"><span>0 best</span><span>10 fastest</span></div>
        </fieldset>
      {/if}

      <fieldset data-tooltip="yuv420p universal · yuv422p broadcast · yuv444p best quality (not always supported)">
        <legend class="text-[12px] font-medium text-[var(--text-secondary)] uppercase tracking-wide mb-1.5">Pixel Format</legend>
        <div class="grid" style="grid-template-columns:repeat(3,1fr)">
          {#each ['yuv420p','yuv422p','yuv444p'] as p, i}
            <button onclick={() => options.pix_fmt = p} class={seg(options.pix_fmt === p, i, 3)}>{p}</button>
          {/each}
        </div>
      </fieldset>
    {/if}

    <!-- ── HAP variant ────────────────────────────────────────────────────── -->
    {#if options.codec === 'hap'}
      <fieldset data-tooltip="HAP — DXT1 no alpha · HAP Alpha — DXT5 with alpha · HAP Q — YCoCg better quality · HAP Q Alpha — YCoCg with alpha">
        <legend class="text-[12px] font-medium text-[var(--text-secondary)] uppercase tracking-wide mb-1.5">HAP Variant</legend>
        <div class="grid" style="grid-template-columns:repeat(4,1fr)">
          {#each [['hap','HAP'],['hap_alpha','HAP α'],['hap_q','HAP Q'],['hap_q_alpha','HAP Qα']] as [v, lbl], i}
            <button onclick={() => options.hap_format = v} class={seg((options.hap_format ?? 'hap') === v, i, 4)}>{lbl}</button>
          {/each}
        </div>
      </fieldset>
    {/if}

    <!-- ── DNxHR profile ──────────────────────────────────────────────────── -->
    {#if options.codec === 'dnxhr'}
      <fieldset data-tooltip="LB low bandwidth · SQ standard · HQ high quality · HQX 12-bit · 444 full 4:4:4 chroma">
        <legend class="text-[12px] font-medium text-[var(--text-secondary)] uppercase tracking-wide mb-1.5">Profile</legend>
        <div class="inline-flex flex-col">
          {#each [['dnxhr_lb','LB — Low Bandwidth'],['dnxhr_sq','SQ — Standard Quality'],['dnxhr_hq','HQ — High Quality'],['dnxhr_hqx','HQX — High Quality 12-bit'],['dnxhr_444','444 — 4:4:4']] as [v, lbl], i}
            <button onclick={() => options.dnxhr_profile = v} class={segV((options.dnxhr_profile ?? 'dnxhr_sq') === v, i, 5)}>{lbl}</button>
          {/each}
        </div>
      </fieldset>
    {/if}

    <!-- ── DNxHD bitrate ──────────────────────────────────────────────────── -->
    {#if options.codec === 'dnxhd'}
      <fieldset data-tooltip="Bitrate must pair with source resolution and frame rate — e.g. 185M for 1080p/29.97, 175M for 1080p/23.976">
        <legend class="text-[12px] font-medium text-[var(--text-secondary)] uppercase tracking-wide mb-1.5">Bitrate</legend>
        <div class="grid" style="grid-template-columns:repeat(4,1fr)">
          {#each [[36,'36M'],[120,'120M'],[175,'175M'],[185,'185M']] as [v, lbl], i}
            <button onclick={() => options.dnxhd_bitrate = v} class={seg((options.dnxhd_bitrate ?? 185) === v, i, 4)}>{lbl}</button>
          {/each}
        </div>
        <p class="text-[11px] text-[var(--text-secondary)] mt-1.5">Mismatched resolution × fps will error. Use DNxHR for resolution-independent encoding.</p>
      </fieldset>
    {/if}

    <!-- ── DV standard ────────────────────────────────────────────────────── -->
    {#if options.codec === 'dvvideo'}
      <fieldset data-tooltip="NTSC: 720×480 at 29.97 fps · PAL: 720×576 at 25 fps — DV forces these exact specs">
        <legend class="text-[12px] font-medium text-[var(--text-secondary)] uppercase tracking-wide mb-1.5">Standard</legend>
        <div class="grid" style="grid-template-columns:repeat(2,1fr)">
          {#each [['ntsc','NTSC (720×480)'],['pal','PAL (720×576)']] as [v, lbl], i}
            <button onclick={() => options.dv_standard = v} class={seg((options.dv_standard ?? 'ntsc') === v, i, 2)}>{lbl}</button>
          {/each}
        </div>
      </fieldset>
    {/if}

    <fieldset data-tooltip="Output frame rate — 24 film · 25 PAL · 30 NTSC · 60 smooth motion / gaming · Orig keeps source">
      <legend class="text-[12px] font-medium text-[var(--text-secondary)] uppercase tracking-wide mb-1.5">Frame Rate</legend>
      <div class="grid" style="grid-template-columns:repeat(5,1fr)">
        {#each ['original','24','25','30','60'] as r, i}
          <button onclick={() => options.frame_rate = r} class={seg(options.frame_rate === r, i, 5)}>{r === 'original' ? 'Orig' : r}</button>
        {/each}
      </div>
    </fieldset>
  {/if}

  {#if options.output_format === 'webm'}
    <fieldset data-tooltip="CRF — quality-based, variable size · CBR — fixed bitrate for streaming · Constrained VBR — VBR capped to a target">
      <legend class="text-[12px] font-medium text-[var(--text-secondary)] uppercase tracking-wide mb-1.5">Bitrate Mode</legend>
      <div class="grid" style="grid-template-columns:repeat(3,1fr)">
        {#each [['crf','CRF'],['cbr','CBR'],['cvbr','Constrained VBR']] as [v, lbl], i}
          <button onclick={() => options.webm_bitrate_mode = v} class={seg(options.webm_bitrate_mode === v, i, 3)}>{lbl}</button>
        {/each}
      </div>
    </fieldset>

    {#if options.webm_bitrate_mode === 'cbr' || options.webm_bitrate_mode === 'cvbr'}
      <fieldset data-tooltip="Target video bitrate for CBR (fixed) or CVBR (capped). Independent of audio bitrate.">
        <legend class="text-[12px] font-medium text-[var(--text-secondary)] uppercase tracking-wide mb-1.5">Video Bitrate — kbps</legend>
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
      <legend class="text-[12px] font-medium text-[var(--text-secondary)] uppercase tracking-wide mb-1.5">Subtitle Track</legend>
      <div class="grid" style="grid-template-columns:repeat(3,1fr)">
        {#each [['none','None'],['copy','Copy'],['burn','Burn-in']] as [v, lbl], i}
          <button onclick={() => options.mkv_subtitle = v} class={seg(options.mkv_subtitle === v, i, 3)}>{lbl}</button>
        {/each}
      </div>
    </fieldset>
  {/if}

  {#if options.output_format === 'avi'}
    <fieldset data-tooltip="Legacy format — no H.265 or modern codec support">
      <legend class="text-[12px] font-medium text-[var(--text-secondary)] uppercase tracking-wide mb-1.5">Video Bitrate — kbps</legend>
      <div class="grid" style="grid-template-columns:repeat(4,1fr)">
        {#each [1000, 4000, 8000, 20000] as b, i}
          <button onclick={() => options.avi_video_bitrate = b} class={seg(options.avi_video_bitrate === b, i, 4)}>{b}</button>
        {/each}
      </div>
    </fieldset>
  {/if}

  {#if options.output_format === 'gif'}
    <fieldset data-tooltip="GIF output width in pixels — height auto-scaled to preserve aspect ratio">
      <legend class="text-[12px] font-medium text-[var(--text-secondary)] uppercase tracking-wide mb-1.5">Output Width (px)</legend>
      <div class="grid" style="grid-template-columns:repeat(4,1fr)">
        {#each [320, 480, 640, 'original'] as w, i}
          <button onclick={() => options.gif_width = w} class={seg(options.gif_width === w, i, 4)}>{w}</button>
        {/each}
      </div>
    </fieldset>
    <fieldset data-tooltip="GIF frame rate — lower = smaller file. 10 fps typical for memes · 15 fps smoother · Orig keeps source rate">
      <legend class="text-[12px] font-medium text-[var(--text-secondary)] uppercase tracking-wide mb-1.5">Frame Rate</legend>
      <div class="grid" style="grid-template-columns:repeat(4,1fr)">
        {#each [5, 10, 15, 'original'] as r, i}
          <button onclick={() => options.gif_fps = r} class={seg(options.gif_fps === r, i, 4)}>{r === 'original' ? 'Orig' : r + ' fps'}</button>
        {/each}
      </div>
    </fieldset>
    <fieldset data-tooltip="Infinite — loop forever · Once — play through then stop · No loop — single play in viewers that honor the flag">
      <legend class="text-[12px] font-medium text-[var(--text-secondary)] uppercase tracking-wide mb-1.5">Loop</legend>
      <div class="grid" style="grid-template-columns:repeat(3,1fr)">
        {#each [['infinite','Infinite'],['once','Once'],['none','No loop']] as [v, lbl], i}
          <button onclick={() => options.gif_loop = v} class={seg(options.gif_loop === v, i, 3)}>{lbl}</button>
        {/each}
      </div>
    </fieldset>
    <fieldset data-tooltip="Max colors in the shared palette. 32/64 smaller file · 256 best color fidelity but largest">
      <legend class="text-[12px] font-medium text-[var(--text-secondary)] uppercase tracking-wide mb-1.5">Palette Size</legend>
      <div class="grid" style="grid-template-columns:repeat(4,1fr)">
        {#each [32, 64, 128, 256] as p, i}
          <button onclick={() => options.gif_palette_size = p} class={seg(options.gif_palette_size === p, i, 4)}>{p}</button>
        {/each}
      </div>
    </fieldset>
    <fieldset data-tooltip="None — flat banding · Bayer — ordered dither, retro look · Floyd-Steinberg — error diffusion, smoothest but busy">
      <legend class="text-[12px] font-medium text-[var(--text-secondary)] uppercase tracking-wide mb-1.5">Dither</legend>
      <div class="grid" style="grid-template-columns:repeat(3,1fr)">
        {#each [['none','None'],['bayer','Bayer'],['floyd','Floyd-Steinberg']] as [v, lbl], i}
          <button onclick={() => options.gif_dither = v} class={seg(options.gif_dither === v, i, 3)}>{lbl}</button>
        {/each}
      </div>
    </fieldset>
  {/if}

  <!-- ── Audio bitrate (segmented) ─────────────────────────────────────── -->
  {#if !options.remove_audio}
    <fieldset data-tooltip="Audio track bitrate — 128 standard · 192 music · 256–320 near-transparent. Independent from video bitrate.">
      <legend class="text-[12px] font-medium text-[var(--text-secondary)] uppercase tracking-wide mb-1.5">
        Audio Bitrate
      </legend>
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
      <legend class="text-[12px] font-medium text-[var(--text-secondary)] uppercase tracking-wide mb-1.5">
        Sample Rate
      </legend>
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

  <label class="inline-flex items-center gap-2.5 cursor-pointer text-[13px]
                bg-[var(--surface-hint)] border border-[var(--border)] rounded-md px-3 py-2
                {options.preserve_metadata ? 'text-[var(--text-primary)]' : 'text-white/75'}"
         data-tooltip="Keep title, encoder, GPS, and other container tags in the output. Uncheck to strip (removes location from phone-recorded video).">
    <input type="checkbox" bind:checked={options.preserve_metadata} class="fade-check" />
    Preserve metadata
  </label>

</div>
