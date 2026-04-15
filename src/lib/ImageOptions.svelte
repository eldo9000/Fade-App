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

  const primaryFormats = ['jpeg', 'png', 'tiff'];
  const extendedFormats = ['webp', 'avif', 'bmp', 'gif', 'heic', 'jp2', 'ico', 'tga', 'jxl'];

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

  // true when the selected format is one of the extended dropdown options
  let dropdownActive = $derived(extendedFormats.includes(options.output_format));

  function selectPrimary(fmt) {
    options.output_format = fmt;
  }

  function onDropdownChange(e) {
    const val = e.currentTarget.value;
    if (val) options.output_format = val;
  }
</script>

<div class="space-y-5" role="form" aria-label="Image conversion options">

  <!-- Output format -->
  <fieldset>
    <legend class="text-[12px] font-medium text-[var(--text-secondary)] uppercase tracking-wide mb-2">
      Output Format
    </legend>
    <div class="flex flex-wrap gap-2 items-center">
      {#each primaryFormats as fmt}
        <button
          onclick={() => selectPrimary(fmt)}
          class="px-3 py-1 rounded text-[12px] font-medium border transition-colors
            {options.output_format === fmt
              ? 'bg-[var(--accent)] text-white border-[var(--accent)]'
              : 'border-[var(--border)] text-[var(--text-primary)] hover:border-[var(--accent)]'}"
        >{fmt.toUpperCase()}</button>
      {/each}
      <select
        value={dropdownActive ? options.output_format : ''}
        onchange={onDropdownChange}
        class="px-2 py-1 rounded text-[12px] border transition-colors outline-none
               bg-[var(--surface)] cursor-pointer
               {dropdownActive
                 ? 'border-[var(--accent)] text-[var(--accent)]'
                 : 'border-[var(--border)] text-[var(--text-primary)] hover:border-[var(--accent)]'}"
      >
        <option value="" disabled>More…</option>
        {#each extendedFormats as fmt}
          <option value={fmt}>{fmt.toUpperCase()}</option>
        {/each}
      </select>
    </div>
  </fieldset>

  <!-- Quality (for lossy formats) -->
  {#if ['jpeg', 'webp', 'avif'].includes(options.output_format)}
    <fieldset>
      <legend class="text-[12px] font-medium text-[var(--text-secondary)] uppercase tracking-wide mb-2">
        Quality — {options.quality}%
      </legend>
      <input
        type="range" min="1" max="100"
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
    <div class="flex gap-2 flex-wrap">
      {#each cropPresets as p}
        {@const isActive = cropActive && (p.ratio === null ? cropAspect === null : cropAspect === p.ratio)}
        <button
          onclick={() => oncropstart?.(p.ratio)}
          class="px-3 py-1 rounded text-[12px] border transition-colors
            {isActive
              ? 'bg-[var(--accent)] text-white border-[var(--accent)]'
              : 'border-[var(--border)] text-[var(--text-primary)] hover:border-[var(--accent)]'}"
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

</div>
