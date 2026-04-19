<script>
  let { codec, output_format, inputWidth = null, inputHeight = null, resolution = $bindable() } = $props();

  function resolveMismatch(target) {
    resolution = (resolution === target) ? 'original' : target;
  }
  function nearestStandardRes(w, h) {
    if (w == null || h == null) return '1920x1080';
    return w >= 1600 ? '1920x1080' : '1280x720';
  }

  function fixBtn(fixed) {
    return 'mt-2 block w-full text-left px-2.5 py-1.5 rounded border text-[11px] font-medium transition-colors ' +
      (fixed
        ? 'border-green-700 bg-green-950/60 text-green-300 hover:bg-green-950/80'
        : 'border-amber-600 bg-amber-900/40 text-amber-200 hover:bg-amber-900/70');
  }
</script>

{#if codec === 'dnxhd'}
  {@const target = (resolution === '1920x1080' || resolution === '1280x720')
    ? resolution
    : ((185 >= 100) ? '1920x1080' : '1280x720')}
  {@const dnxhdTarget = '1920x1080'}
  {@const mismatch = inputWidth != null && inputHeight != null &&
    (inputWidth !== parseInt(dnxhdTarget) || inputHeight !== parseInt(dnxhdTarget.split('x')[1]))}
  {@const fixed = resolution === dnxhdTarget}
  <div class="rounded-md border border-amber-800 bg-amber-950/40 px-4 py-3 text-[12px] text-amber-300 leading-relaxed">
    <strong class="text-amber-200 block mb-1">DNxHD: bitrate must match resolution × fps exactly.</strong>
    185M → 1080p/29.97 · 175M → 1080p/23.976 · 120M → 1080p low · 220M → 720p/59.94 · 36M → any/offline.
    ffmpeg will error if the combination doesn't match. Use <strong class="text-amber-200">DNxHR</strong> if you're unsure — it works at any resolution.
    {#if mismatch || fixed}
      <button onclick={() => resolveMismatch(dnxhdTarget)} class={fixBtn(fixed)}>
        {#if fixed}✓ Scaling to {dnxhdTarget} — click to undo
        {:else}⚠ Input is {inputWidth}×{inputHeight} — Scale to {dnxhdTarget} →{/if}
      </button>
    {/if}
  </div>

{:else if codec === 'cineform'}
  {@const cfTarget = nearestStandardRes(inputWidth, inputHeight)}
  {@const mismatch = inputWidth != null && inputHeight != null &&
    (inputWidth % 16 !== 0 || inputHeight % 16 !== 0)}
  {@const fixed = resolution === cfTarget && mismatch}
  <div class="rounded-md border border-amber-800 bg-amber-950/40 px-4 py-3 text-[12px] text-amber-300 leading-relaxed">
    <strong class="text-amber-200 block mb-1">CineForm requires the cfhd encoder — not in standard ffmpeg builds.</strong>
    If you see <em>"Encoder cfhd not found"</em>, your ffmpeg was built without GoPro CineForm support.
    Install a build that includes it, or use <strong class="text-amber-200">DNxHR HQ</strong> as a lossless-quality alternative.
    {#if mismatch || fixed}
      <button onclick={() => resolveMismatch(cfTarget)} class={fixBtn(fixed)}>
        {#if fixed}✓ Scaling to {cfTarget} — click to undo
        {:else}⚠ Input {inputWidth}×{inputHeight} is not mod-16 — Scale to {cfTarget} →{/if}
      </button>
    {/if}
  </div>

{:else if codec === 'dvvideo'}
  <div class="rounded-md border border-amber-800 bg-amber-950/40 px-4 py-3 text-[12px] text-amber-300 leading-relaxed">
    <strong class="text-amber-200 block mb-1">DV forces exact resolution and frame rate.</strong>
    Source video will be scaled and resampled — NTSC to 720×480/29.97, PAL to 720×576/25.
    Audio is resampled to 48 kHz. DV is a tape-era format; use H.264 for anything modern.
  </div>

{:else if codec === 'rawvideo'}
  <div class="rounded-md border border-amber-800 bg-amber-950/40 px-4 py-3 text-[12px] text-amber-300 leading-relaxed">
    <strong class="text-amber-200 block mb-1">Uncompressed video produces very large files.</strong>
    1 minute of 1080p yuv422p ≈ 12 GB · 4K yuv422p ≈ 48 GB. Make sure you have enough disk space before converting.
  </div>

{:else if codec === 'wmv2'}
  <div class="rounded-md border border-amber-800 bg-amber-950/40 px-4 py-3 text-[12px] text-amber-300 leading-relaxed">
    <strong class="text-amber-200 block mb-1">WMV / ASF is a legacy Windows Media format.</strong>
    WMV2 video and WMA2 audio are used automatically. Quality is poor compared to H.264 at the same file size.
    Only use this if you specifically need Windows Media compatibility.
  </div>

{:else if codec === 'rv20'}
  <div class="rounded-md border border-red-900 bg-red-950/40 px-4 py-3 text-[12px] text-red-300 leading-relaxed">
    <strong class="text-red-200 block mb-1">RealMedia encoding is not supported by most ffmpeg builds.</strong>
    Standard Homebrew ffmpeg cannot write RMVB. This will likely error with <em>"muxer not found"</em> or
    <em>"Encoder rv20 not found"</em>. ffmpeg can read RMVB files but writing requires a custom build with RealMedia support.
  </div>

{:else if codec === 'xdcam422'}
  {@const mismatch = inputWidth != null && inputHeight != null && (inputWidth !== 1920 || inputHeight !== 1080)}
  {@const fixed = resolution === '1920x1080'}
  <div class="rounded-md border border-amber-800 bg-amber-950/40 px-4 py-3 text-[12px] text-amber-300 leading-relaxed">
    <strong class="text-amber-200 block mb-1">XDCAM HD422 requires 1920×1080, 50 Mbps CBR, yuv422p.</strong>
    These are set automatically. Intended for Sony XDCAM ingest workflows — use MXF or MOV container.
    {#if mismatch || fixed}
      <button onclick={() => resolveMismatch('1920x1080')} class={fixBtn(fixed)}>
        {#if fixed}✓ Scaling to 1920×1080 — click to undo
        {:else}⚠ Input is {inputWidth}×{inputHeight} — Scale to 1920×1080 →{/if}
      </button>
    {/if}
  </div>

{:else if codec === 'xdcam35'}
  {@const mismatch = inputWidth != null && inputHeight != null && (inputWidth !== 1440 || inputHeight !== 1080)}
  {@const fixed = resolution === '1440x1080'}
  <div class="rounded-md border border-amber-800 bg-amber-950/40 px-4 py-3 text-[12px] text-amber-300 leading-relaxed">
    <strong class="text-amber-200 block mb-1">XDCAM HD35 requires 1440×1080, 35 Mbps CBR, yuv420p.</strong>
    These are set automatically. The 1440×1080 frame stores 16:9 content using non-square (anamorphic) pixels.
    {#if mismatch || fixed}
      <button onclick={() => resolveMismatch('1440x1080')} class={fixBtn(fixed)}>
        {#if fixed}✓ Scaling to 1440×1080 — click to undo
        {:else}⚠ Input is {inputWidth}×{inputHeight} — Scale to 1440×1080 →{/if}
      </button>
    {/if}
  </div>

{:else if output_format === '3gp'}
  <div class="rounded-md border border-amber-800 bg-amber-950/40 px-4 py-3 text-[12px] text-amber-300 leading-relaxed">
    <strong class="text-amber-200 block mb-1">3GP is a mobile container — H.264 Baseline has been set automatically.</strong>
    For older phones, scale to 480p or lower in the Resolution control in the sidebar. Audio is AAC LC.
  </div>

{:else if output_format === 'divx'}
  <div class="rounded-md border border-amber-800 bg-amber-950/40 px-4 py-3 text-[12px] text-amber-300 leading-relaxed">
    <strong class="text-amber-200 block mb-1">DivX uses MPEG-4 video in an AVI-compatible container.</strong>
    MPEG-4 codec has been set automatically. Compatible with legacy DivX-certified players and set-top boxes.
  </div>
{/if}
