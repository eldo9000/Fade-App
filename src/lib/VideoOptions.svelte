<script>
  let { options = $bindable(), errors = {} } = $props();

  const codecs  = [
    { value: 'copy',  label: 'Copy' },
    { value: 'h264',  label: 'H.264' },
    { value: 'h265',  label: 'H.265' },
    { value: 'h266',  label: 'H.266', todo: true },
    { value: 'vp8',   label: 'VP8',   todo: true },
    { value: 'vp9',   label: 'VP9' },
    { value: 'av1',   label: 'AV1' },
  ];
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

  // shared segmented-control button classes
  function seg(active, i, total) {
    const base = 'px-3 py-1.5 text-center text-[12px] font-medium border transition-colors relative';
    const round = i === 0 ? 'rounded-l-md' : i === total - 1 ? 'rounded-r-md' : '';
    const overlap = i > 0 ? '-ml-px' : '';
    const color = active
      ? 'bg-[var(--accent)] text-white border-[var(--accent)] z-10'
      : 'border-[var(--border)] text-[var(--text-primary)] hover:z-10 hover:border-[var(--accent)] hover:text-[var(--accent)]';
    return [base, round, overlap, color].filter(Boolean).join(' ');
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

  // Vertical connected row (for Audio Track and similar)
  function segV(active, i, total) {
    const base  = 'w-full px-3 py-1.5 text-left text-[12px] font-medium border transition-colors relative';
    const round = i === 0 ? 'rounded-t-md' : i === total - 1 ? 'rounded-b-md' : '';
    const mt    = i > 0 ? '-mt-px' : '';
    const color = active
      ? 'bg-[var(--accent)] text-white border-[var(--accent)] z-10'
      : 'border-[var(--border)] text-[var(--text-primary)] hover:z-10 hover:border-[var(--accent)] hover:text-[var(--accent)]';
    return [base, round, mt, color].filter(Boolean).join(' ');
  }
</script>

<div class="space-y-5" role="form" aria-label="Video conversion options">

  <!-- ── Codec (button group) ─────────────────────────────────────────── -->
  <fieldset data-tooltip="Video encoding codec — H.264 for compatibility, H.265/AV1 for smaller files">
    <legend class="text-[12px] font-medium text-[var(--text-secondary)] uppercase tracking-wide mb-2">
      Video Codec
    </legend>
    <div class="flex flex-wrap gap-1">
      {#each codecs.filter(c => !c.todo || import.meta.env.DEV) as c}
        <button
          onclick={() => { if (!c.todo) options.codec = c.value; }}
          class="px-2 py-0.5 rounded text-[11px] font-mono border transition-colors
                 {options.codec === c.value
                   ? 'bg-[var(--accent)] text-white border-[var(--accent)]'
                   : c.todo
                     ? 'border-green-900 text-green-400 hover:border-green-600 hover:bg-green-950'
                     : 'border-[var(--border)] text-[var(--text-primary)] hover:border-[var(--accent)] hover:text-[var(--accent)]'}"
        >{c.label}</button>
      {/each}
    </div>
  </fieldset>

  <!-- ── Resolution (segmented) ────────────────────────────────────────── -->
  <fieldset data-tooltip="Scale the output video resolution">
    <legend class="text-[12px] font-medium text-[var(--text-secondary)] uppercase tracking-wide mb-2">
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
  <fieldset data-tooltip="Keep, strip, or extract the audio track from the video">
    <legend class="text-[12px] font-medium text-[var(--text-secondary)] uppercase tracking-wide mb-2">
      Audio Track
    </legend>
    <div class="flex flex-col">
      <button onclick={() => { options.remove_audio = false; options.extract_audio = false; }}
              class={segV(!options.remove_audio && !options.extract_audio, 0, 3)}>
        Keep audio
      </button>
      <button onclick={() => { options.remove_audio = true; options.extract_audio = false; }}
              class={segV(options.remove_audio, 1, 3)}>
        Remove audio
      </button>
      <button onclick={() => { options.extract_audio = true; options.remove_audio = false; }}
              class={segV(options.extract_audio, 2, 3)}>
        Extract audio only
      </button>
    </div>
    {#if options.extract_audio}
      <div class="grid mt-2" style="grid-template-columns: repeat({audioFormats.length}, 1fr)">
        {#each audioFormats as fmt, i}
          <button onclick={() => options.audio_format = fmt}
                  class={seg(options.audio_format === fmt, i, audioFormats.length)}
          >{fmt}</button>
        {/each}
      </div>
    {/if}
  </fieldset>

  <!-- ── Trim ───────────────────────────────────────────────────────────── -->
  <fieldset data-tooltip="Trim the output — enter time as MM:SS or raw seconds.">
    <legend class="text-[12px] font-medium text-[var(--text-secondary)] uppercase tracking-wide mb-2">
      Trim
    </legend>
    <div class="flex gap-3 items-end">
      <div class="flex-1">
        <label class="text-[11px] text-[var(--text-secondary)]" for="vid-trim-start">Start</label>
        <input id="vid-trim-start" type="text" placeholder=""
          value={trimStartRaw} oninput={onTrimStartInput}
          class="w-full mt-1 px-3 py-1.5 rounded-md border border-[var(--border)]
                 bg-[var(--surface)] text-[var(--text-primary)] text-[13px]
                 focus:outline-none focus:border-[var(--accent)]"
        />
      </div>
      <div class="flex-1">
        <label class="text-[11px] text-[var(--text-secondary)]" for="vid-trim-end">End</label>
        <input id="vid-trim-end" type="text" placeholder=""
          value={trimEndRaw} oninput={onTrimEndInput}
          class="w-full mt-1 px-3 py-1.5 rounded-md text-[13px]
                 focus:outline-none focus:border-[var(--accent)]
                 bg-[var(--surface)] text-[var(--text-primary)]
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
      <fieldset data-tooltip="0 lossless · 18–28 typical · 51 worst · lower = better quality">
        <legend class="text-[12px] font-medium text-[var(--text-secondary)] uppercase tracking-wide mb-2">Quality — CRF {options.crf}</legend>
        <input type="range" min="0" max="51" step="1" bind:value={options.crf} class="w-full accent-[var(--accent)]" />
        <div class="flex justify-between text-[10px] text-[var(--text-secondary)] mt-1"><span>0 lossless</span><span>51 worst</span></div>
      </fieldset>

      <fieldset data-tooltip="Slower preset = better compression at same quality">
        <legend class="text-[12px] font-medium text-[var(--text-secondary)] uppercase tracking-wide mb-2">Encode Preset</legend>
        <div class="flex flex-col">
          {#each ['ultrafast','fast','medium','slow','veryslow'] as p, i}
            <button onclick={() => options.preset = p} class={segV(options.preset === p, i, 5)}>{p}</button>
          {/each}
        </div>
      </fieldset>

      {#if options.codec === 'h264' || options.codec === 'h265'}
        <fieldset data-tooltip="baseline — max compatibility · main — consumer · high — streaming / archival">
          <legend class="text-[12px] font-medium text-[var(--text-secondary)] uppercase tracking-wide mb-2">Profile</legend>
          <div class="grid" style="grid-template-columns:repeat(3,1fr)">
            {#each ['baseline','main','high'] as p, i}
              <button onclick={() => options.h264_profile = p} class={seg(options.h264_profile === p, i, 3)}>{p}</button>
            {/each}
          </div>
        </fieldset>
        <fieldset data-tooltip="none for general use · film / animation / grain / stillimage optimize for content type">
          <legend class="text-[12px] font-medium text-[var(--text-secondary)] uppercase tracking-wide mb-2">Tune</legend>
          <div class="grid" style="grid-template-columns:repeat(4,1fr)">
            {#each ['none','film','animation','grain'] as t, i}
              <button onclick={() => options.tune = t} class={seg(options.tune === t, i, 4)}>{t}</button>
            {/each}
          </div>
        </fieldset>
      {/if}

      {#if options.codec === 'vp9'}
        <fieldset data-tooltip="VP9 speed/deadline: 0 best quality · 5 fastest">
          <legend class="text-[12px] font-medium text-[var(--text-secondary)] uppercase tracking-wide mb-2">Speed — {options.vp9_speed}</legend>
          <input type="range" min="0" max="5" step="1" bind:value={options.vp9_speed} class="w-full accent-[var(--accent)]" />
          <div class="flex justify-between text-[10px] text-[var(--text-secondary)] mt-1"><span>0 best</span><span>5 fastest</span></div>
        </fieldset>
      {/if}

      {#if options.codec === 'av1'}
        <fieldset data-tooltip="AV1 cpu-used: 0 slowest / best · 10 fastest / worst">
          <legend class="text-[12px] font-medium text-[var(--text-secondary)] uppercase tracking-wide mb-2">Speed — {options.av1_speed}</legend>
          <input type="range" min="0" max="10" step="1" bind:value={options.av1_speed} class="w-full accent-[var(--accent)]" />
          <div class="flex justify-between text-[10px] text-[var(--text-secondary)] mt-1"><span>0 best</span><span>10 fastest</span></div>
        </fieldset>
      {/if}

      <fieldset data-tooltip="yuv420p universal · yuv422p broadcast · yuv444p best quality (not always supported)">
        <legend class="text-[12px] font-medium text-[var(--text-secondary)] uppercase tracking-wide mb-2">Pixel Format</legend>
        <div class="grid" style="grid-template-columns:repeat(3,1fr)">
          {#each ['yuv420p','yuv422p','yuv444p'] as p, i}
            <button onclick={() => options.pix_fmt = p} class={seg(options.pix_fmt === p, i, 3)}>{p}</button>
          {/each}
        </div>
      </fieldset>
    {/if}

    <fieldset>
      <legend class="text-[12px] font-medium text-[var(--text-secondary)] uppercase tracking-wide mb-2">Frame Rate</legend>
      <div class="grid" style="grid-template-columns:repeat(5,1fr)">
        {#each ['original','24','25','30','60'] as r, i}
          <button onclick={() => options.frame_rate = r} class={seg(options.frame_rate === r, i, 5)}>{r === 'original' ? 'Orig' : r}</button>
        {/each}
      </div>
    </fieldset>
  {/if}

  {#if options.output_format === 'webm'}
    <fieldset>
      <legend class="text-[12px] font-medium text-[var(--text-secondary)] uppercase tracking-wide mb-2">Bitrate Mode</legend>
      <div class="grid" style="grid-template-columns:repeat(3,1fr)">
        {#each [['crf','CRF'],['cbr','CBR'],['cvbr','Constrained VBR']] as [v, lbl], i}
          <button onclick={() => options.webm_bitrate_mode = v} class={seg(options.webm_bitrate_mode === v, i, 3)}>{lbl}</button>
        {/each}
      </div>
    </fieldset>
  {/if}

  {#if options.output_format === 'mkv'}
    <fieldset>
      <legend class="text-[12px] font-medium text-[var(--text-secondary)] uppercase tracking-wide mb-2">Subtitle Track</legend>
      <div class="grid" style="grid-template-columns:repeat(3,1fr)">
        {#each [['none','None'],['copy','Copy'],['burn','Burn-in']] as [v, lbl], i}
          <button onclick={() => options.mkv_subtitle = v} class={seg(options.mkv_subtitle === v, i, 3)}>{lbl}</button>
        {/each}
      </div>
    </fieldset>
  {/if}

  {#if options.output_format === 'avi'}
    <fieldset data-tooltip="Legacy format — no H.265 or modern codec support">
      <legend class="text-[12px] font-medium text-[var(--text-secondary)] uppercase tracking-wide mb-2">Video Bitrate — kbps</legend>
      <div class="grid" style="grid-template-columns:repeat(4,1fr)">
        {#each [1000, 4000, 8000, 20000] as b, i}
          <button onclick={() => options.avi_video_bitrate = b} class={seg(options.avi_video_bitrate === b, i, 4)}>{b}</button>
        {/each}
      </div>
    </fieldset>
  {/if}

  {#if options.output_format === 'gif'}
    <fieldset data-tooltip="Height auto-scaled to preserve aspect ratio">
      <legend class="text-[12px] font-medium text-[var(--text-secondary)] uppercase tracking-wide mb-2">Output Width (px)</legend>
      <div class="grid" style="grid-template-columns:repeat(4,1fr)">
        {#each [320, 480, 640, 'original'] as w, i}
          <button onclick={() => options.gif_width = w} class={seg(options.gif_width === w, i, 4)}>{w}</button>
        {/each}
      </div>
    </fieldset>
    <fieldset>
      <legend class="text-[12px] font-medium text-[var(--text-secondary)] uppercase tracking-wide mb-2">Frame Rate</legend>
      <div class="grid" style="grid-template-columns:repeat(4,1fr)">
        {#each [5, 10, 15, 'original'] as r, i}
          <button onclick={() => options.gif_fps = r} class={seg(options.gif_fps === r, i, 4)}>{r === 'original' ? 'Orig' : r + ' fps'}</button>
        {/each}
      </div>
    </fieldset>
    <fieldset>
      <legend class="text-[12px] font-medium text-[var(--text-secondary)] uppercase tracking-wide mb-2">Loop</legend>
      <div class="grid" style="grid-template-columns:repeat(3,1fr)">
        {#each [['infinite','Infinite'],['once','Once'],['none','No loop']] as [v, lbl], i}
          <button onclick={() => options.gif_loop = v} class={seg(options.gif_loop === v, i, 3)}>{lbl}</button>
        {/each}
      </div>
    </fieldset>
    <fieldset>
      <legend class="text-[12px] font-medium text-[var(--text-secondary)] uppercase tracking-wide mb-2">Palette Size</legend>
      <div class="grid" style="grid-template-columns:repeat(4,1fr)">
        {#each [32, 64, 128, 256] as p, i}
          <button onclick={() => options.gif_palette_size = p} class={seg(options.gif_palette_size === p, i, 4)}>{p}</button>
        {/each}
      </div>
    </fieldset>
    <fieldset>
      <legend class="text-[12px] font-medium text-[var(--text-secondary)] uppercase tracking-wide mb-2">Dither</legend>
      <div class="grid" style="grid-template-columns:repeat(3,1fr)">
        {#each [['none','None'],['bayer','Bayer'],['floyd','Floyd-Steinberg']] as [v, lbl], i}
          <button onclick={() => options.gif_dither = v} class={seg(options.gif_dither === v, i, 3)}>{lbl}</button>
        {/each}
      </div>
    </fieldset>
  {/if}

  <!-- ── Audio bitrate (segmented) ─────────────────────────────────────── -->
  {#if !options.remove_audio}
    <fieldset>
      <legend class="text-[12px] font-medium text-[var(--text-secondary)] uppercase tracking-wide mb-2">
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

    <fieldset>
      <legend class="text-[12px] font-medium text-[var(--text-secondary)] uppercase tracking-wide mb-2">
        Sample Rate
      </legend>
      <div class="flex flex-col">
        {#each sampleRates as sr, i}
          <button onclick={() => options.sample_rate = sr.value}
                  class={segV(options.sample_rate === sr.value, i, sampleRates.length)}>
            {sr.label}
          </button>
        {/each}
      </div>
    </fieldset>
  {/if}

  <label class="flex items-center gap-2 cursor-pointer"
         data-tooltip="Keep title, encoder, GPS, and other container tags in the output. Uncheck to strip (removes location from phone-recorded video).">
    <input type="checkbox" bind:checked={options.preserve_metadata} class="accent-[var(--accent)]" />
    <span class="text-[12px] text-[var(--text-primary)]">Preserve metadata</span>
  </label>

</div>
