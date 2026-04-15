<script>
  let { options = $bindable() } = $props();

  const formats = ['webp','jpeg','png','avif','tiff','bmp','gif'];
  const resizeModes = [
    { value: 'none',    label: 'No resize' },
    { value: 'percent', label: 'Percentage' },
    { value: 'pixels',  label: 'Pixel dimensions' },
  ];

  const aspectPresets = [
    { label: '1:1',   ratio: 1 },
    { label: '4:3',   ratio: 4/3 },
    { label: '16:9',  ratio: 16/9 },
    { label: '3:2',   ratio: 3/2 },
    { label: '21:9',  ratio: 21/9 },
  ];

  let cropEnabled = $derived(!!(options.crop_width && options.crop_height));

  function applyCropAspect(ratio) {
    const w = options.crop_width || 800;
    options.crop_width = w;
    options.crop_height = Math.round(w / ratio);
  }

  function clearCrop() {
    options.crop_x = 0;
    options.crop_y = 0;
    options.crop_width = null;
    options.crop_height = null;
  }
</script>

<div class="space-y-5" role="form" aria-label="Image conversion options">

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

  <!-- Quality (for lossy formats) -->
  {#if ['webp','jpeg','avif'].includes(options.output_format)}
    <fieldset>
      <legend class="text-[12px] font-medium text-[var(--text-secondary)] uppercase tracking-wide mb-2">
        Quality — {options.quality}%
      </legend>
      <input
        type="range" min="1" max="100"
        bind:value={options.quality}
        class="w-full accent-[var(--accent)]"
        aria-label="Quality {options.quality}%"
      />
      <div class="flex justify-between text-[11px] text-[var(--text-secondary)] mt-1">
        <span>Smaller file</span><span>Higher quality</span>
      </div>
    </fieldset>
  {/if}

  <!-- Rotation & flip -->
  <fieldset>
    <legend class="text-[12px] font-medium text-[var(--text-secondary)] uppercase tracking-wide mb-2">
      Rotation & Orientation
    </legend>
    <div class="flex flex-wrap gap-2 mb-2">
      {#each [0, 90, 180, 270] as deg}
        <button
          onclick={() => options.rotation = deg}
          class="px-3 py-1 rounded text-[12px] border transition-colors
            {options.rotation === deg
              ? 'bg-[var(--accent)] text-white border-[var(--accent)]'
              : 'border-[var(--border)] text-[var(--text-primary)] hover:border-[var(--accent)]'}"
        >{deg === 0 ? 'No rotate' : deg + '°'}</button>
      {/each}
    </div>
    <div class="flex flex-wrap gap-2">
      <button
        onclick={() => options.flip_h = !options.flip_h}
        class="px-3 py-1 rounded text-[12px] border transition-colors
          {options.flip_h
            ? 'bg-[var(--accent)] text-white border-[var(--accent)]'
            : 'border-[var(--border)] text-[var(--text-primary)] hover:border-[var(--accent)]'}"
      >Mirror horizontal</button>
      <button
        onclick={() => options.flip_v = !options.flip_v}
        class="px-3 py-1 rounded text-[12px] border transition-colors
          {options.flip_v
            ? 'bg-[var(--accent)] text-white border-[var(--accent)]'
            : 'border-[var(--border)] text-[var(--text-primary)] hover:border-[var(--accent)]'}"
      >Mirror vertical</button>
    </div>
    <label class="flex items-center gap-2 mt-2 cursor-pointer">
      <input type="checkbox" bind:checked={options.auto_rotate} class="accent-[var(--accent)]" />
      <span class="text-[12px] text-[var(--text-primary)]">Auto-rotate from EXIF</span>
    </label>
  </fieldset>

  <!-- Crop -->
  <fieldset>
    <legend class="text-[12px] font-medium text-[var(--text-secondary)] uppercase tracking-wide mb-2">
      Crop
    </legend>
    <div class="flex gap-2 flex-wrap mb-2">
      {#each aspectPresets as p}
        <button
          onclick={() => applyCropAspect(p.ratio)}
          class="px-3 py-1 rounded text-[12px] border border-[var(--border)]
                 text-[var(--text-primary)] hover:border-[var(--accent)] transition-colors"
        >{p.label}</button>
      {/each}
      {#if cropEnabled}
        <button
          onclick={clearCrop}
          class="px-3 py-1 rounded text-[12px] border border-[var(--border)]
                 text-red-500 hover:border-red-400 transition-colors"
        >Clear</button>
      {/if}
    </div>
    <div class="grid grid-cols-2 gap-3">
      <div>
        <label class="text-[11px] text-[var(--text-secondary)]" for="crop-w">Width px</label>
        <input id="crop-w" type="number" min="1"
          bind:value={options.crop_width}
          placeholder="Full width"
          class="w-full mt-1 px-3 py-1.5 rounded-md border border-[var(--border)]
                 bg-[var(--surface)] text-[var(--text-primary)] text-[13px]
                 focus:outline-none focus:border-[var(--accent)]"
        />
      </div>
      <div>
        <label class="text-[11px] text-[var(--text-secondary)]" for="crop-h">Height px</label>
        <input id="crop-h" type="number" min="1"
          bind:value={options.crop_height}
          placeholder="Full height"
          class="w-full mt-1 px-3 py-1.5 rounded-md border border-[var(--border)]
                 bg-[var(--surface)] text-[var(--text-primary)] text-[13px]
                 focus:outline-none focus:border-[var(--accent)]"
        />
      </div>
      <div>
        <label class="text-[11px] text-[var(--text-secondary)]" for="crop-x">Offset X px</label>
        <input id="crop-x" type="number" min="0"
          bind:value={options.crop_x}
          class="w-full mt-1 px-3 py-1.5 rounded-md border border-[var(--border)]
                 bg-[var(--surface)] text-[var(--text-primary)] text-[13px]
                 focus:outline-none focus:border-[var(--accent)]"
        />
      </div>
      <div>
        <label class="text-[11px] text-[var(--text-secondary)]" for="crop-y">Offset Y px</label>
        <input id="crop-y" type="number" min="0"
          bind:value={options.crop_y}
          class="w-full mt-1 px-3 py-1.5 rounded-md border border-[var(--border)]
                 bg-[var(--surface)] text-[var(--text-primary)] text-[13px]
                 focus:outline-none focus:border-[var(--accent)]"
        />
      </div>
    </div>
    {#if !cropEnabled}
      <p class="text-[11px] text-[var(--text-secondary)] mt-1.5">Leave blank to skip cropping.</p>
    {/if}
  </fieldset>

  <!-- Resize mode -->
  <fieldset>
    <legend class="text-[12px] font-medium text-[var(--text-secondary)] uppercase tracking-wide mb-2">
      Resize
    </legend>
    <select
      bind:value={options.resize_mode}
      class="w-full px-3 py-1.5 rounded-md border border-[var(--border)]
             bg-[var(--surface)] text-[var(--text-primary)] text-[13px]
             focus:outline-none focus:border-[var(--accent)]"
    >
      {#each resizeModes as m}
        <option value={m.value}>{m.label}</option>
      {/each}
    </select>
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

  <!-- Common presets -->
  <fieldset>
    <legend class="text-[12px] font-medium text-[var(--text-secondary)] uppercase tracking-wide mb-2">
      Quick Presets
    </legend>
    <div class="flex flex-wrap gap-2">
      {#each [
        { label: 'Web thumb', w: 200, h: 200, fmt: 'webp', q: 80 },
        { label: 'Twitter header', w: 1500, h: 500, fmt: 'jpeg', q: 90 },
        { label: 'Discord avatar', w: 512, h: 512, fmt: 'png', q: null },
        { label: 'Half size', pct: 50, fmt: null, q: null },
      ] as preset}
        <button
          onclick={() => {
            if (preset.fmt) options.output_format = preset.fmt;
            if (preset.q != null) options.quality = preset.q;
            if (preset.pct) {
              options.resize_mode = 'percent';
              options.resize_percent = preset.pct;
            } else {
              options.resize_mode = 'pixels';
              options.resize_width = preset.w;
              options.resize_height = preset.h;
            }
          }}
          class="px-3 py-1 rounded text-[12px] border border-[var(--border)]
                 text-[var(--text-primary)] hover:border-[var(--accent)] transition-colors"
        >{preset.label}</button>
      {/each}
    </div>
  </fieldset>

</div>
