<script>
  let { options = $bindable(), errors = {} } = $props();

  const formats = ['mp4','mkv','webm','avi','mov'];
  const codecs  = [
    { value: 'copy',  label: 'Stream copy (fast, lossless)' },
    { value: 'h264',  label: 'H.264 — max compatibility' },
    { value: 'h265',  label: 'H.265 — smaller files' },
    { value: 'vp9',   label: 'VP9 — open source' },
    { value: 'av1',   label: 'AV1 — best compression (slow)' },
  ];
  const resolutions = [
    { value: 'original',  label: 'Original' },
    { value: '1920x1080', label: '1080p' },
    { value: '1280x720',  label: '720p' },
    { value: '854x480',   label: '480p' },
  ];
  const audioBitrates = [64, 128, 192, 256, 320];
  const sampleRates   = [
    { value: 44100, label: '44.1 kHz (CD)' },
    { value: 48000, label: '48 kHz (video standard)' },
    { value: 96000, label: '96 kHz (Hi-Fi)' },
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

  <!-- ── Output format (segmented) ─────────────────────────────────────── -->
  <fieldset data-tooltip="Choose the output container format">
    <legend class="text-[12px] font-medium text-[var(--text-secondary)] uppercase tracking-wide mb-2">
      Output Format
    </legend>
    <div class="grid" style="grid-template-columns: repeat({formats.length}, 1fr)">
      {#each formats as fmt, i}
        <button onclick={() => options.output_format = fmt}
                data-tooltip="Convert to {fmt.toUpperCase()}"
                class={seg(options.output_format === fmt, i, formats.length)}
        >{fmt.toUpperCase()}</button>
      {/each}
    </div>
  </fieldset>

  <!-- ── Codec (dropdown) ───────────────────────────────────────────────── -->
  <fieldset data-tooltip="Video encoding codec — H.264 for compatibility, H.265/AV1 for smaller files">
    <legend class="text-[12px] font-medium text-[var(--text-secondary)] uppercase tracking-wide mb-2">
      Video Codec
    </legend>
    <select
      bind:value={options.codec}
      data-tooltip="Stream copy skips re-encoding (lossless, instant)"
      class="w-full px-3 py-1.5 rounded-md border border-[var(--border)]
             bg-[var(--surface)] text-[var(--text-primary)] text-[13px]
             focus:outline-none focus:border-[var(--accent)]"
    >
      {#each codecs as c}
        <option value={c.value}>{c.label}</option>
      {/each}
    </select>
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
  <fieldset data-tooltip="Trim the output — enter time as MM:SS or raw seconds. Leave blank to keep full duration.">
    <legend class="text-[12px] font-medium text-[var(--text-secondary)] uppercase tracking-wide mb-2">
      Trim (MM:SS or seconds)
    </legend>
    <div class="flex gap-3 items-end">
      <div class="flex-1">
        <label class="text-[11px] text-[var(--text-secondary)]" for="vid-trim-start">Start</label>
        <input id="vid-trim-start" type="text" placeholder="0:00"
          value={trimStartRaw} oninput={onTrimStartInput}
          class="w-full mt-1 px-3 py-1.5 rounded-md border border-[var(--border)]
                 bg-[var(--surface)] text-[var(--text-primary)] text-[13px]
                 focus:outline-none focus:border-[var(--accent)]"
        />
      </div>
      <div class="flex-1">
        <label class="text-[11px] text-[var(--text-secondary)]" for="vid-trim-end">End</label>
        <input id="vid-trim-end" type="text" placeholder="end"
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
    {:else}
      <p class="text-[11px] text-[var(--text-secondary)] mt-1.5">Leave blank to keep full duration.</p>
    {/if}
  </fieldset>

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
  {/if}

</div>
