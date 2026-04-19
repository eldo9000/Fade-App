<script>
  let {
    options = $bindable(),
    onqualitystart = null,
    onqualityinput = null,
    onqualityend   = null,
    oncropstart    = null,
    oncropclear    = null,
    cropActive     = false,
    cropAspect     = null,
  } = $props();

  const resizeModes = [
    { value: 'none',    label: 'No resize' },
    { value: 'percent', label: 'Percentage' },
    { value: 'pixels',  label: 'Pixel dimensions' },
  ];

  const cropPresets = [
    { label: 'Free',  ratio: null },
    { label: '1:1',   ratio: 1 },
    { label: '4:3',   ratio: 4/3 },
    { label: '16:9',  ratio: 16/9 },
    { label: '3:2',   ratio: 3/2 },
    { label: '21:9',  ratio: 21/9 },
  ];

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

  function seg(active, i, total) {
    const base  = 'px-3 py-1.5 text-center text-[12px] font-medium border transition-colors relative';
    const round = i === 0 ? 'rounded-l-md' : i === total - 1 ? 'rounded-r-md' : '';
    const ml    = i > 0 ? '-ml-px' : '';
    const color = active
      ? 'bg-[var(--accent)] text-white border-[var(--accent)] z-10'
      : 'border-[var(--border)] text-[var(--text-primary)] hover:z-10 hover:border-[var(--accent)] hover:text-[var(--accent)]';
    return [base, round, ml, color].filter(Boolean).join(' ');
  }

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

<div class="space-y-5" role="form" aria-label="Image conversion options">

  <!-- Quality (for lossy formats) -->
  {#if ['jpeg', 'webp', 'avif'].includes(options.output_format)}
    <fieldset data-tooltip="Lossy compression quality — 80–95 typical for photos · 100 near-lossless · below 60 visible artifacts">
      <legend class="text-[12px] font-medium text-[var(--text-secondary)] uppercase tracking-wide mb-2">
        Quality — {options.quality}%
      </legend>
      <input
        type="range" min="5" max="100" step="5"
        bind:value={options.quality}
        class="w-full accent-[var(--accent)]"
        aria-label="Quality {options.quality}%"
        onmousedown={onqualitystart}
        oninput={onqualityinput}
      />
      <div class="flex justify-between text-[11px] text-[var(--text-secondary)] mt-1">
        <span>Smaller file</span><span>Higher quality</span>
      </div>
    </fieldset>
  {/if}

  <!-- Crop -->
  <fieldset data-tooltip="Click a ratio then drag on the preview to crop. Free = any aspect · 1:1 square · 16:9 widescreen · 3:2 DSLR · 21:9 ultrawide.">
    <legend class="text-[12px] font-medium text-[var(--text-secondary)] uppercase tracking-wide mb-2">
      Crop
    </legend>
    <div class="grid" style="grid-template-columns: repeat({cropPresets.length}, 1fr)">
      {#each cropPresets as p, i}
        {@const isActive = cropActive && (p.ratio === null ? cropAspect === null : cropAspect === p.ratio)}
        <button
          onclick={() => oncropstart?.(p.ratio)}
          class={seg(isActive, i, cropPresets.length)}
        >{p.label}</button>
      {/each}
    </div>
    {#if options.crop_width != null}
      <div class="flex items-center gap-2 mt-2">
        <span class="font-mono text-[11px] text-[var(--text-secondary)]">
          {options.crop_width} × {options.crop_height} px
        </span>
        <button
          onclick={() => oncropclear?.()}
          class="px-2 py-0.5 rounded text-[11px] border border-red-700 text-red-400
                 hover:border-red-500 hover:bg-red-900/20 transition-colors"
        >Clear crop</button>
      </div>
    {/if}
  </fieldset>

  <!-- Resize mode -->
  <fieldset data-tooltip="Scale the output. Percentage scales uniformly · Pixel dimensions lets you set exact W×H · No resize keeps original size.">
    <legend class="text-[12px] font-medium text-[var(--text-secondary)] uppercase tracking-wide mb-2">
      Resize
    </legend>
    <div class="flex flex-col">
      {#each resizeModes as m, i}
        <button onclick={() => options.resize_mode = m.value} class={segV(options.resize_mode === m.value, i, resizeModes.length)}>{m.label}</button>
      {/each}
    </div>
  </fieldset>

  {#if options.resize_mode === 'percent'}
    <fieldset data-tooltip="Uniform scale factor — 50% halves dimensions · 200% doubles · aspect ratio always preserved">
      <legend class="text-[12px] font-medium text-[var(--text-secondary)] uppercase tracking-wide mb-2">
        Scale — {options.resize_percent}%
      </legend>
      <input
        type="range" min="1" max="400"
        bind:value={options.resize_percent}
        class="w-full accent-[var(--accent)]"
        aria-label="Scale {options.resize_percent}%"
      />
      <div class="flex justify-between text-[11px] text-[var(--text-secondary)] mt-1">
        <span>1%</span><span>400%</span>
      </div>
    </fieldset>
  {/if}

  {#if options.resize_mode === 'pixels'}
    <fieldset data-tooltip="Set exact output dimensions in pixels. Leave one side at 0 to auto-compute it and keep aspect ratio.">
      <legend class="text-[12px] font-medium text-[var(--text-secondary)] uppercase tracking-wide mb-2">
        Dimensions (0 = auto)
      </legend>
      <div class="flex gap-3 items-center">
        <div class="flex-1">
          <label class="text-[11px] text-[var(--text-secondary)]" for="img-w">Width px</label>
          <input id="img-w" type="number" min="0"
            bind:value={options.resize_width}
            class="w-full mt-1 px-3 py-1.5 rounded-md border border-[var(--border)]
                   bg-[var(--surface)] text-[var(--text-primary)] text-[13px]
                   focus:outline-none focus:border-[var(--accent)]"
          />
        </div>
        <span class="text-[var(--text-secondary)] mt-4">×</span>
        <div class="flex-1">
          <label class="text-[11px] text-[var(--text-secondary)]" for="img-h">Height px</label>
          <input id="img-h" type="number" min="0"
            bind:value={options.resize_height}
            class="w-full mt-1 px-3 py-1.5 rounded-md border border-[var(--border)]
                   bg-[var(--surface)] text-[var(--text-primary)] text-[13px]
                   focus:outline-none focus:border-[var(--accent)]"
          />
        </div>
      </div>
      <p class="text-[11px] text-[var(--text-secondary)] mt-1">Aspect ratio preserved when one dimension is 0.</p>
    </fieldset>
  {/if}

  <!-- ── Format-specific controls ──────────────────────────────────────── -->

  {#if options.output_format === 'jpeg'}
    <fieldset data-tooltip="4:4:4 for text / print / high detail · 4:2:0 for photos / smallest file">
      <legend class="text-[12px] font-medium text-[var(--text-secondary)] uppercase tracking-wide mb-2">Chroma Subsampling</legend>
      <div class="grid" style="grid-template-columns:repeat(3,1fr)">
        {#each [['420','4:2:0'],['422','4:2:2'],['444','4:4:4']] as [v, lbl], i}
          <button onclick={() => options.jpeg_chroma = v} class={seg(options.jpeg_chroma === v, i, 3)}>{lbl}</button>
        {/each}
      </div>
    </fieldset>
    <label class="flex items-center gap-2 cursor-pointer"
           data-tooltip="Progressive JPEG loads top-to-bottom in multiple passes on slow connections — slightly smaller file, broadly supported.">
      <input type="checkbox" bind:checked={options.jpeg_progressive} class="accent-[var(--accent)]" />
      <span class="text-[12px] text-[var(--text-primary)]">Progressive JPEG</span>
    </label>

  {:else if options.output_format === 'png'}
    <fieldset data-tooltip="0 none · 9 max · always lossless">
      <legend class="text-[12px] font-medium text-[var(--text-secondary)] uppercase tracking-wide mb-2">Compression — {options.png_compression}</legend>
      <input type="range" min="0" max="9" step="1" bind:value={options.png_compression} class="w-full accent-[var(--accent)]" />
      <div class="flex justify-between text-[10px] text-[var(--text-secondary)] mt-1"><span>0 fastest</span><span>9 smallest</span></div>
    </fieldset>
    <fieldset data-tooltip="RGB/RGBA full color · Grayscale shrinks file ~3× · Palette 8-bit smallest for few colors / line art">
      <legend class="text-[12px] font-medium text-[var(--text-secondary)] uppercase tracking-wide mb-2">Color Mode</legend>
      <div class="flex flex-col">
        {#each [['rgb','RGB'],['rgba','RGBA'],['gray','Grayscale'],['graya','Grayscale + Alpha'],['palette','Palette (8-bit)']] as [v, lbl], i}
          <button onclick={() => options.png_color_mode = v} class={segV(options.png_color_mode === v, i, 5)}>{lbl}</button>
        {/each}
      </div>
    </fieldset>
    <label class="flex items-center gap-2 cursor-pointer"
           data-tooltip="Adam7 interlacing — image loads as progressively refined passes. Slightly larger file, useful for slow connections.">
      <input type="checkbox" bind:checked={options.png_interlaced} class="accent-[var(--accent)]" />
      <span class="text-[12px] text-[var(--text-primary)]">Interlaced (Adam7)</span>
    </label>

  {:else if options.output_format === 'tiff'}
    <fieldset data-tooltip="LZW widely supported lossless · Deflate (zip) best ratio · PackBits legacy fast · None largest but max compatibility">
      <legend class="text-[12px] font-medium text-[var(--text-secondary)] uppercase tracking-wide mb-2">Compression</legend>
      <div class="grid" style="grid-template-columns:repeat(4,1fr)">
        {#each [['none','None'],['lzw','LZW'],['deflate','Deflate'],['packbits','PackBits']] as [v, lbl], i}
          <button onclick={() => options.tiff_compression = v} class={seg(options.tiff_compression === v, i, 4)}>{lbl}</button>
        {/each}
      </div>
    </fieldset>
    <fieldset data-tooltip="8-bit standard · 16-bit for HDR / color grading · 32-bit float for scientific imaging">
      <legend class="text-[12px] font-medium text-[var(--text-secondary)] uppercase tracking-wide mb-2">Bit Depth</legend>
      <div class="grid" style="grid-template-columns:repeat(3,1fr)">
        {#each [[8,'8-bit'],[16,'16-bit'],[32,'32-bit float']] as [v, lbl], i}
          <button onclick={() => options.tiff_bit_depth = v} class={seg(options.tiff_bit_depth === v, i, 3)}>{lbl}</button>
        {/each}
      </div>
    </fieldset>
    <fieldset data-tooltip="CMYK for print workflows">
      <legend class="text-[12px] font-medium text-[var(--text-secondary)] uppercase tracking-wide mb-2">Color Mode</legend>
      <div class="grid" style="grid-template-columns:repeat(3,1fr)">
        {#each [['rgb','RGB'],['cmyk','CMYK'],['gray','Grayscale']] as [v, lbl], i}
          <button onclick={() => options.tiff_color_mode = v} class={seg(options.tiff_color_mode === v, i, 3)}>{lbl}</button>
        {/each}
      </div>
    </fieldset>

  {:else if options.output_format === 'webp'}
    <label class="flex items-center gap-2 cursor-pointer"
           data-tooltip="Encode WebP without perceptual loss — larger files, pixel-perfect. Disables Quality; alpha channel always preserved.">
      <input type="checkbox" bind:checked={options.webp_lossless} class="accent-[var(--accent)]" />
      <span class="text-[12px] text-[var(--text-primary)]">Lossless mode</span>
    </label>
    <fieldset data-tooltip="0 fastest · 6 best compression (slower encode)">
      <legend class="text-[12px] font-medium text-[var(--text-secondary)] uppercase tracking-wide mb-2">Compression Method — {options.webp_method}</legend>
      <input type="range" min="0" max="6" step="1" bind:value={options.webp_method} class="w-full accent-[var(--accent)]" />
      <div class="flex justify-between text-[10px] text-[var(--text-secondary)] mt-1"><span>0 fastest</span><span>6 best</span></div>
    </fieldset>

  {:else if options.output_format === 'avif'}
    <fieldset data-tooltip="0 slowest / best · 10 fastest / worst">
      <legend class="text-[12px] font-medium text-[var(--text-secondary)] uppercase tracking-wide mb-2">Speed — {options.avif_speed}</legend>
      <input type="range" min="0" max="10" step="1" bind:value={options.avif_speed} class="w-full accent-[var(--accent)]" />
      <div class="flex justify-between text-[10px] text-[var(--text-secondary)] mt-1"><span>0 best</span><span>10 fastest</span></div>
    </fieldset>
    <fieldset data-tooltip="4:2:0 smallest file · 4:2:2 broadcast quality · 4:4:4 full chroma for text / graphics">
      <legend class="text-[12px] font-medium text-[var(--text-secondary)] uppercase tracking-wide mb-2">Chroma</legend>
      <div class="grid" style="grid-template-columns:repeat(3,1fr)">
        {#each [['420','YUV 4:2:0'],['422','YUV 4:2:2'],['444','YUV 4:4:4']] as [v, lbl], i}
          <button onclick={() => options.avif_chroma = v} class={seg(options.avif_chroma === v, i, 3)}>{lbl}</button>
        {/each}
      </div>
    </fieldset>

  {:else if options.output_format === 'bmp'}
    <fieldset data-tooltip="Uncompressed — large files · legacy Windows format">
      <legend class="text-[12px] font-medium text-[var(--text-secondary)] uppercase tracking-wide mb-2">Bit Depth</legend>
      <div class="grid" style="grid-template-columns:repeat(4,1fr)">
        {#each [[8,'8-bit'],[16,'16-bit'],[24,'24-bit'],[32,'32-bit']] as [v, lbl], i}
          <button onclick={() => options.bmp_bit_depth = v} class={seg(options.bmp_bit_depth === v, i, 4)}>{lbl}</button>
        {/each}
      </div>
    </fieldset>
  {/if}

  <!-- Rotation & flip -->
  <fieldset data-tooltip="Rotate clockwise in 90° steps, and/or flip the image. Combine with Auto-rotate to correct phone photos.">
    <legend class="text-[12px] font-medium text-[var(--text-secondary)] uppercase tracking-wide mb-2">
      Rotation & Orientation
    </legend>
    <div class="grid mb-2" style="grid-template-columns: repeat(4, 1fr)"
         data-tooltip="Fixed-angle rotation applied to the output — clockwise">
      {#each [0, 90, 180, 270] as deg, i}
        <button
          onclick={() => options.rotation = deg}
          data-tooltip={deg === 0 ? 'Leave orientation unchanged' : `Rotate ${deg}° clockwise`}
          class={seg(options.rotation === deg, i, 4)}
        >{deg === 0 ? 'None' : deg + '°'}</button>
      {/each}
    </div>
    <div class="grid grid-cols-2 mb-2">
      <button
        onclick={() => options.flip_h = !options.flip_h}
        data-tooltip="Mirror horizontally — swap left and right"
        class={seg(options.flip_h, 0, 2)}
      >Flip H</button>
      <button
        onclick={() => options.flip_v = !options.flip_v}
        data-tooltip="Mirror vertically — swap top and bottom"
        class={seg(options.flip_v, 1, 2)}
      >Flip V</button>
    </div>
    <label class="flex items-center gap-2 mt-2 cursor-pointer"
           data-tooltip="Read the EXIF orientation tag from the source and rotate to match. Fixes sideways photos from phones without re-rotating.">
      <input type="checkbox" bind:checked={options.auto_rotate} class="accent-[var(--accent)]" />
      <span class="text-[12px] text-[var(--text-primary)]">Auto-rotate from EXIF</span>
    </label>
    <label class="flex items-center gap-2 mt-2 cursor-pointer"
           data-tooltip="Keep EXIF, ICC profile, and other metadata in the output. Uncheck to strip (removes GPS, camera info, timestamps).">
      <input type="checkbox" bind:checked={options.preserve_metadata} class="accent-[var(--accent)]" />
      <span class="text-[12px] text-[var(--text-primary)]">Preserve metadata</span>
    </label>
  </fieldset>

</div>
