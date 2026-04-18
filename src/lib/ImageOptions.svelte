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
    <fieldset>
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
  <fieldset>
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
  <fieldset>
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
    <fieldset>
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
    <fieldset>
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

  <!-- ── Format-specific controls (dev) ──────────────────────────────────── -->
  {#if import.meta.env.DEV}
    <div class="flex items-center gap-2">
      <div class="flex-1 h-px bg-green-900/50"></div>
      <span class="text-[9px] text-green-500/70 uppercase tracking-widest font-mono shrink-0">format-specific · dev</span>
      <div class="flex-1 h-px bg-green-900/50"></div>
    </div>

    {#if options.output_format === 'jpeg'}
      <fieldset>
        <legend class="text-[12px] font-medium text-green-400 uppercase tracking-wide mb-2">Chroma Subsampling</legend>
        <div class="grid" style="grid-template-columns:repeat(3,1fr)">
          {#each ['4:2:0','4:2:2','4:4:4'] as s, i}<button class={devSeg(i,3)}>{s}</button>{/each}
        </div>
        <p class="text-[10px] text-green-500/60 mt-1">4:4:4 for text / print / high detail</p>
      </fieldset>
      <fieldset>
        <legend class="text-[12px] font-medium text-green-400 uppercase tracking-wide mb-2">Progressive</legend>
        <div class="grid" style="grid-template-columns:repeat(2,1fr)">
          {#each ['No','Yes'] as v, i}<button class={devSeg(i,2)}>{v}</button>{/each}
        </div>
      </fieldset>

    {:else if options.output_format === 'png'}
      <fieldset>
        <legend class="text-[12px] font-medium text-green-400 uppercase tracking-wide mb-2">Compression Level</legend>
        <div class="grid" style="grid-template-columns:repeat(5,1fr)">
          {#each [0,2,4,6,9] as l, i}<button class={devSeg(i,5)}>{l}</button>{/each}
        </div>
        <p class="text-[10px] text-green-500/60 mt-1">0 none · 9 max · lossless either way</p>
      </fieldset>
      <fieldset>
        <legend class="text-[12px] font-medium text-green-400 uppercase tracking-wide mb-2">Color Mode</legend>
        <div class="flex flex-col">
          {#each ['RGB','RGBA','Grayscale','Grayscale + Alpha','Palette (8-bit)'] as m, i}
            <button class={devSegV(i,5)}>{m}</button>
          {/each}
        </div>
      </fieldset>
      <fieldset>
        <legend class="text-[12px] font-medium text-green-400 uppercase tracking-wide mb-2">Interlaced</legend>
        <div class="grid" style="grid-template-columns:repeat(2,1fr)">
          {#each ['No','Yes'] as v, i}<button class={devSeg(i,2)}>{v}</button>{/each}
        </div>
      </fieldset>

    {:else if options.output_format === 'tiff'}
      <fieldset>
        <legend class="text-[12px] font-medium text-green-400 uppercase tracking-wide mb-2">Compression</legend>
        <div class="grid" style="grid-template-columns:repeat(4,1fr)">
          {#each ['None','LZW','Deflate','PackBits'] as c, i}<button class={devSeg(i,4)}>{c}</button>{/each}
        </div>
      </fieldset>
      <fieldset>
        <legend class="text-[12px] font-medium text-green-400 uppercase tracking-wide mb-2">Bit Depth</legend>
        <div class="grid" style="grid-template-columns:repeat(3,1fr)">
          {#each ['8-bit','16-bit','32-bit float'] as d, i}<button class={devSeg(i,3)}>{d}</button>{/each}
        </div>
      </fieldset>
      <fieldset>
        <legend class="text-[12px] font-medium text-green-400 uppercase tracking-wide mb-2">Color Mode</legend>
        <div class="grid" style="grid-template-columns:repeat(3,1fr)">
          {#each ['RGB','CMYK','Grayscale'] as m, i}<button class={devSeg(i,3)}>{m}</button>{/each}
        </div>
        <p class="text-[10px] text-green-500/60 mt-1">CMYK for print workflows</p>
      </fieldset>

    {:else if options.output_format === 'webp'}
      <fieldset>
        <legend class="text-[12px] font-medium text-green-400 uppercase tracking-wide mb-2">Mode</legend>
        <div class="grid" style="grid-template-columns:repeat(2,1fr)">
          {#each ['Lossy','Lossless'] as m, i}<button class={devSeg(i,2)}>{m}</button>{/each}
        </div>
      </fieldset>
      <fieldset>
        <legend class="text-[12px] font-medium text-green-400 uppercase tracking-wide mb-2">Compression Effort</legend>
        <div class="grid" style="grid-template-columns:repeat(4,1fr)">
          {#each [0,2,4,6] as e, i}<button class={devSeg(i,4)}>{e}</button>{/each}
        </div>
        <p class="text-[10px] text-green-500/60 mt-1">0 fastest · 6 best compression</p>
      </fieldset>

    {:else if options.output_format === 'avif'}
      <fieldset>
        <legend class="text-[12px] font-medium text-green-400 uppercase tracking-wide mb-2">Encode Effort</legend>
        <div class="grid" style="grid-template-columns:repeat(5,1fr)">
          {#each [0,2,4,6,10] as e, i}<button class={devSeg(i,5)}>{e}</button>{/each}
        </div>
        <p class="text-[10px] text-green-500/60 mt-1">0 fastest · 10 best — encodes slowly</p>
      </fieldset>
      <fieldset>
        <legend class="text-[12px] font-medium text-green-400 uppercase tracking-wide mb-2">Color Space</legend>
        <div class="grid" style="grid-template-columns:repeat(3,1fr)">
          {#each ['YUV 4:2:0','YUV 4:2:2','YUV 4:4:4'] as s, i}<button class={devSeg(i,3)}>{s}</button>{/each}
        </div>
      </fieldset>

    {:else if options.output_format === 'bmp'}
      <fieldset>
        <legend class="text-[12px] font-medium text-green-400 uppercase tracking-wide mb-2">Bit Depth</legend>
        <div class="grid" style="grid-template-columns:repeat(4,1fr)">
          {#each ['8-bit','16-bit','24-bit','32-bit'] as d, i}<button class={devSeg(i,4)}>{d}</button>{/each}
        </div>
        <p class="text-[10px] text-green-500/60 mt-1">Uncompressed — large files · legacy Windows format</p>
      </fieldset>

    {:else if options.output_format === 'gif'}
      <fieldset>
        <legend class="text-[12px] font-medium text-green-400 uppercase tracking-wide mb-2">Palette Size</legend>
        <div class="grid" style="grid-template-columns:repeat(4,1fr)">
          {#each [32,64,128,256] as p, i}<button class={devSeg(i,4)}>{p}</button>{/each}
        </div>
      </fieldset>
      <fieldset>
        <legend class="text-[12px] font-medium text-green-400 uppercase tracking-wide mb-2">Dither</legend>
        <div class="grid" style="grid-template-columns:repeat(3,1fr)">
          {#each ['None','Bayer','Floyd-Steinberg'] as d, i}<button class={devSeg(i,3)}>{d}</button>{/each}
        </div>
      </fieldset>
      <fieldset>
        <legend class="text-[12px] font-medium text-green-400 uppercase tracking-wide mb-2">Loop</legend>
        <div class="grid" style="grid-template-columns:repeat(3,1fr)">
          {#each ['Infinite','Once','No loop'] as l, i}<button class={devSeg(i,3)}>{l}</button>{/each}
        </div>
      </fieldset>

    {:else if options.output_format === 'heic'}
      <fieldset>
        <legend class="text-[12px] font-medium text-green-400 uppercase tracking-wide mb-2">Bit Depth</legend>
        <div class="grid" style="grid-template-columns:repeat(3,1fr)">
          {#each ['8-bit','10-bit','12-bit'] as d, i}<button class={devSeg(i,3)}>{d}</button>{/each}
        </div>
        <p class="text-[10px] text-green-500/60 mt-1">~40% smaller than JPEG · limited non-Apple decoder support</p>
      </fieldset>

    {:else if options.output_format === 'jp2'}
      <fieldset>
        <legend class="text-[12px] font-medium text-green-400 uppercase tracking-wide mb-2">Compression Mode</legend>
        <div class="grid" style="grid-template-columns:repeat(2,1fr)">
          {#each ['Lossless','Lossy'] as m, i}<button class={devSeg(i,2)}>{m}</button>{/each}
        </div>
        <p class="text-[10px] text-green-500/60 mt-1">Archival / medical imaging — poor browser support</p>
      </fieldset>
      <fieldset>
        <legend class="text-[12px] font-medium text-green-400 uppercase tracking-wide mb-2">Compression Ratio (lossy)</legend>
        <div class="grid" style="grid-template-columns:repeat(4,1fr)">
          {#each ['5:1','10:1','20:1','40:1'] as r, i}<button class={devSeg(i,4)}>{r}</button>{/each}
        </div>
      </fieldset>

    {:else if options.output_format === 'ico'}
      <fieldset>
        <legend class="text-[12px] font-medium text-green-400 uppercase tracking-wide mb-2">Sizes to Include</legend>
        <div class="flex flex-wrap gap-1">
          {#each ['16×16','32×32','48×48','64×64','128×128','256×256'] as s}
            <button class="px-2 py-0.5 rounded text-[11px] font-mono border border-green-900 text-green-400 hover:border-green-700 hover:bg-green-950/40 transition-colors">{s}</button>
          {/each}
        </div>
        <p class="text-[10px] text-green-500/60 mt-1">256×256 stored as PNG internally</p>
      </fieldset>
      <fieldset>
        <legend class="text-[12px] font-medium text-green-400 uppercase tracking-wide mb-2">Bit Depth</legend>
        <div class="grid" style="grid-template-columns:repeat(4,1fr)">
          {#each ['4-bit','8-bit','24-bit','32-bit'] as d, i}<button class={devSeg(i,4)}>{d}</button>{/each}
        </div>
      </fieldset>

    {:else if options.output_format === 'tga'}
      <fieldset>
        <legend class="text-[12px] font-medium text-green-400 uppercase tracking-wide mb-2">Bit Depth</legend>
        <div class="grid" style="grid-template-columns:repeat(3,1fr)">
          {#each ['16-bit','24-bit','32-bit'] as d, i}<button class={devSeg(i,3)}>{d}</button>{/each}
        </div>
        <p class="text-[10px] text-green-500/60 mt-1">32-bit supports alpha channel</p>
      </fieldset>
      <fieldset>
        <legend class="text-[12px] font-medium text-green-400 uppercase tracking-wide mb-2">RLE Compression</legend>
        <div class="grid" style="grid-template-columns:repeat(2,1fr)">
          {#each ['No','Yes'] as v, i}<button class={devSeg(i,2)}>{v}</button>{/each}
        </div>
      </fieldset>
    {/if}
  {/if}

  <!-- Rotation & flip -->
  <fieldset>
    <legend class="text-[12px] font-medium text-[var(--text-secondary)] uppercase tracking-wide mb-2">
      Rotation & Orientation
    </legend>
    <div class="grid mb-2" style="grid-template-columns: repeat(4, 1fr)">
      {#each [0, 90, 180, 270] as deg, i}
        <button
          onclick={() => options.rotation = deg}
          class={seg(options.rotation === deg, i, 4)}
        >{deg === 0 ? 'None' : deg + '°'}</button>
      {/each}
    </div>
    <div class="grid grid-cols-2 mb-2">
      <button
        onclick={() => options.flip_h = !options.flip_h}
        class={seg(options.flip_h, 0, 2)}
      >Flip H</button>
      <button
        onclick={() => options.flip_v = !options.flip_v}
        class={seg(options.flip_v, 1, 2)}
      >Flip V</button>
    </div>
    <label class="flex items-center gap-2 mt-2 cursor-pointer">
      <input type="checkbox" bind:checked={options.auto_rotate} class="accent-[var(--accent)]" />
      <span class="text-[12px] text-[var(--text-primary)]">Auto-rotate from EXIF</span>
    </label>
  </fieldset>

</div>
