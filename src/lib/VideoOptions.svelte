<script>
  let { options = $bindable(), errors = {} } = $props();

  const formats = ['mp4','mkv','webm','avi','mov'];
  const codecs = [
    { value: 'copy',  label: 'Stream copy (fast, lossless)' },
    { value: 'h264',  label: 'H.264 — max compatibility' },
    { value: 'h265',  label: 'H.265 — smaller files' },
    { value: 'vp9',   label: 'VP9 — open source' },
    { value: 'av1',   label: 'AV1 — best compression (slow)' },
  ];
  const resolutions = [
    { value: 'original', label: 'Original' },
    { value: '1920x1080', label: '1080p — Full HD' },
    { value: '1280x720',  label: '720p — HD' },
    { value: '854x480',   label: '480p — SD' },
  ];
  const audioBitrates = [64, 128, 192, 256, 320];
  const sampleRates = [
    { value: 44100, label: '44.1 kHz (CD)' },
    { value: 48000, label: '48 kHz (video standard)' },
    { value: 96000, label: '96 kHz (Hi-Fi)' },
  ];

  // Parse "MM:SS" or bare seconds into a float. Returns null for empty input.
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

  // Format seconds as MM:SS for display in the input.
  function formatTime(secs) {
    if (secs == null) return '';
    const m = Math.floor(secs / 60);
    const s = (secs % 60).toFixed(1);
    return `${m}:${s.padStart(4, '0')}`;
  }

  // Derived from options so timeline drags update the inputs automatically
  let trimStartRaw = $derived(options.trim_start != null ? formatTime(options.trim_start) : '');
  let trimEndRaw   = $derived(options.trim_end   != null ? formatTime(options.trim_end)   : '');

  function onTrimStartInput(e) { options.trim_start = parseTime(e.target.value); }
  function onTrimEndInput(e)   { options.trim_end   = parseTime(e.target.value); }
  function clearTrim() {
    options.trim_start = null;
    options.trim_end = null;
  }

  // YouTube / Discord / Twitter presets
  const presets = [
    { label: 'YouTube 1080p', fmt: 'mp4', codec: 'h264', res: '1920x1080', br: 192, sr: 48000 },
    { label: 'YouTube 720p',  fmt: 'mp4', codec: 'h264', res: '1280x720',  br: 128, sr: 48000 },
    { label: 'Discord',       fmt: 'mp4', codec: 'h264', res: '1280x720',  br: 128, sr: 48000 },
    { label: 'Twitter',       fmt: 'mp4', codec: 'h264', res: '1920x1080', br: 192, sr: 48000 },
    { label: 'Rewrap only',   fmt: 'mp4', codec: 'copy', res: 'original',  br: null, sr: null },
  ];

  function applyPreset(p) {
    options.output_format = p.fmt;
    options.codec = p.codec;
    options.resolution = p.res;
    if (p.br != null) options.bitrate = p.br;
    if (p.sr != null) options.sample_rate = p.sr;
  }
</script>

<div class="space-y-5" role="form" aria-label="Video conversion options">

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

  <!-- Codec -->
  <fieldset>
    <legend class="text-[12px] font-medium text-[var(--text-secondary)] uppercase tracking-wide mb-2">
      Video Codec
    </legend>
    <select
      bind:value={options.codec}
      class="w-full px-3 py-1.5 rounded-md border border-[var(--border)]
             bg-[var(--surface)] text-[var(--text-primary)] text-[13px]
             focus:outline-none focus:border-[var(--accent)]"
    >
      {#each codecs as c}
        <option value={c.value}>{c.label}</option>
      {/each}
    </select>
  </fieldset>

  <!-- Resolution -->
  <fieldset>
    <legend class="text-[12px] font-medium text-[var(--text-secondary)] uppercase tracking-wide mb-2">
      Resolution
    </legend>
    <div class="flex flex-wrap gap-2">
      {#each resolutions as r}
        <button
          onclick={() => options.resolution = r.value}
          class="px-3 py-1 rounded text-[12px] border transition-colors
            {options.resolution === r.value
              ? 'bg-[var(--accent)] text-white border-[var(--accent)]'
              : 'border-[var(--border)] text-[var(--text-primary)] hover:border-[var(--accent)]'}"
        >{r.label}</button>
      {/each}
    </div>
    {#if errors.resolution}
      <p class="text-[11px] text-red-500 mt-1.5">{errors.resolution}</p>
    {/if}
  </fieldset>

  <!-- Audio track options -->
  <fieldset>
    <legend class="text-[12px] font-medium text-[var(--text-secondary)] uppercase tracking-wide mb-2">
      Audio Track
    </legend>
    <div class="space-y-1.5">
      <label class="flex items-center gap-2 text-[13px] text-[var(--text-primary)] cursor-pointer">
        <input type="radio" name="audio-track" value="keep"
          checked={!options.remove_audio && !options.extract_audio}
          onchange={() => { options.remove_audio = false; options.extract_audio = false; }}
          class="accent-[var(--accent)]"
        /> Keep audio
      </label>
      <label class="flex items-center gap-2 text-[13px] text-[var(--text-primary)] cursor-pointer">
        <input type="radio" name="audio-track" value="remove"
          checked={options.remove_audio}
          onchange={() => { options.remove_audio = true; options.extract_audio = false; }}
          class="accent-[var(--accent)]"
        /> Remove audio (video only)
      </label>
      <label class="flex items-center gap-2 text-[13px] text-[var(--text-primary)] cursor-pointer">
        <input type="radio" name="audio-track" value="extract"
          checked={options.extract_audio}
          onchange={() => { options.extract_audio = true; options.remove_audio = false; }}
          class="accent-[var(--accent)]"
        /> Extract audio only
      </label>
    </div>
    {#if options.extract_audio}
      <div class="mt-2 flex gap-2">
        {#each ['mp3','wav','flac','aac','opus'] as fmt}
          <button
            onclick={() => options.audio_format = fmt}
            class="px-2 py-0.5 rounded text-[12px] border transition-colors
              {options.audio_format === fmt
                ? 'bg-[var(--accent)] text-white border-[var(--accent)]'
                : 'border-[var(--border)] text-[var(--text-primary)] hover:border-[var(--accent)]'}"
          >{fmt}</button>
        {/each}
      </div>
    {/if}
  </fieldset>

  <!-- Trim -->
  <fieldset>
    <legend class="text-[12px] font-medium text-[var(--text-secondary)] uppercase tracking-wide mb-2">
      Trim (MM:SS or seconds)
    </legend>
    <div class="flex gap-3 items-end">
      <div class="flex-1">
        <label class="text-[11px] text-[var(--text-secondary)]" for="vid-trim-start">Start</label>
        <input id="vid-trim-start" type="text" placeholder="0:00"
          value={trimStartRaw}
          oninput={onTrimStartInput}
          class="w-full mt-1 px-3 py-1.5 rounded-md border border-[var(--border)]
                 bg-[var(--surface)] text-[var(--text-primary)] text-[13px]
                 focus:outline-none focus:border-[var(--accent)]"
        />
      </div>
      <div class="flex-1">
        <label class="text-[11px] text-[var(--text-secondary)]" for="vid-trim-end">End</label>
        <input id="vid-trim-end" type="text" placeholder="end"
          value={trimEndRaw}
          oninput={onTrimEndInput}
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

  <!-- Audio bitrate -->
  {#if !options.remove_audio}
    <fieldset>
      <legend class="text-[12px] font-medium text-[var(--text-secondary)] uppercase tracking-wide mb-2">
        Audio Bitrate
      </legend>
      <div class="flex gap-2 flex-wrap">
        {#each audioBitrates as br}
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
  {/if}

</div>
