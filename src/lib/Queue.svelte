<script>
  let { queue, selectedId, onselect, onremove, oncancel, oninfo, compatibleTypes = [], compact = false } = $props();

  function statusColor(status) {
    switch (status) {
      case 'done':       return 'text-green-500';
      case 'error':      return 'text-red-500';
      case 'cancelled':  return 'text-orange-400';
      case 'converting': return 'text-[var(--accent)]';
      default:           return 'text-[var(--text-secondary)]';
    }
  }

  function statusIcon(status) {
    switch (status) {
      case 'done':       return '✓';
      case 'error':      return '✕';
      case 'cancelled':  return '⊘';
      case 'converting': return '↻';
      default:           return '·';
    }
  }

  let expandedErrors = $state(new Set());

  function toggleError(id) {
    const next = new Set(expandedErrors);
    if (next.has(id)) next.delete(id);
    else next.add(id);
    expandedErrors = next;
  }

  function fileStats(item) {
    const info = item.info;
    const mt = item.mediaType;
    if (!info) return null;
    if (mt === 'video') {
      const res  = info.width && info.height ? `${info.width}×${info.height}` : null;
      const codec = info.codec ? info.codec.toUpperCase() : null;
      return [res, codec].filter(Boolean).join(' · ') || null;
    }
    if (mt === 'audio') {
      const codec = info.codec ? info.codec.toUpperCase() : null;
      const fmt   = info.format ? info.format.toUpperCase() : null;
      // Show codec; if it differs from the container format, show both
      if (codec && fmt && codec !== fmt) return `${codec} / ${fmt}`;
      return codec ?? fmt ?? null;
    }
    if (mt === 'image') {
      return info.width && info.height ? `${info.width}×${info.height}` : null;
    }
    return null;
  }
</script>

<!-- File list -->
<div class="flex-1 min-h-0 overflow-y-auto" role="list" aria-label="Files in queue">
  {#if queue.length === 0}
    <div class="h-full overflow-y-auto px-4 py-3 flex flex-col gap-0">

      <!-- Drop hint -->
      <div class="flex flex-col items-center gap-1.5 py-4 mb-3 border border-dashed border-[var(--border)]
                  rounded-lg text-center">
        <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor"
             stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"
             class="text-[var(--border)]">
          <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"/>
          <polyline points="17 8 12 3 7 8"/>
          <line x1="12" y1="3" x2="12" y2="15"/>
        </svg>
        <p class="text-[11px] text-[var(--text-secondary)]">Drop files or click Browse</p>
      </div>

      <!-- Supported types -->
      {#each [
        { label: 'Image',            exts: 'jpg  jpeg  png  gif  webp  avif  bmp  svg  ico' },
        { label: 'RAW & Pro Image',  exts: 'heic  heif  tiff  psd  raw  cr2  cr3  nef  arw  dng  orf  rw2  exr  hdr  dds  xcf' },
        { label: 'Video',            exts: 'mp4  m4v  mkv  webm  mov  avi  flv  wmv  mpg  mpeg  ogv  ts  3gp  divx  rmvb  asf' },
        { label: 'Audio',            exts: 'mp3  aac  ogg  wav  flac  m4a  opus  wma  aiff  alac  ac3  dts' },
        { label: 'Document',         exts: 'pdf' },
        { label: '3D Model',         exts: 'obj  gltf  glb  stl  fbx  ply  3ds' },
      ] as group}
        <div class="py-2.5 border-b border-[var(--border)] last:border-0">
          <p class="text-[11px] font-semibold text-[var(--text-primary)] mb-1">{group.label}</p>
          <p class="text-[10px] text-[var(--text-secondary)] leading-relaxed font-mono tracking-wide">{group.exts}</p>
        </div>
      {/each}
    </div>
  {:else}
    {#each queue as item (item.id)}
      <!-- svelte-ignore a11y_click_events_have_key_events -->
      <!-- svelte-ignore a11y_no_static_element_interactions -->
      <div
        role="listitem"
        onclick={() => onselect?.(item.id)}
        class="relative overflow-hidden flex {compact ? 'items-center' : 'items-start'} gap-2 px-3 {compact ? 'py-1' : 'py-2'} border-b border-[var(--border)] group cursor-pointer transition-colors
               {selectedId === item.id ? '' : 'hover:bg-[var(--surface)]'}
               {compatibleTypes.length > 0 && !compatibleTypes.includes(item.mediaType) ? 'opacity-40' : ''}"
        style={selectedId === item.id
          ? 'background:color-mix(in srgb,var(--accent) 12%,transparent); border-left:2px solid var(--accent); padding-left:10px'
          : ''}
      >
        {#if item.status === 'converting'}
          <div class="absolute inset-0 pointer-events-none transition-all duration-300"
               style="background: linear-gradient(to right, color-mix(in srgb, var(--accent) 18%, transparent) {item.percent ?? 0}%, transparent {item.percent ?? 0}%)">
          </div>
        {/if}

        {#if compact}
          <!-- Compact: filename only -->
          <p class="flex-1 min-w-0 text-[12px] text-white truncate leading-tight" title={item.path}>{item.name}</p>
          <button
            onclick={(e) => { e.stopPropagation(); onremove?.(item.id); }}
            class="w-5 h-5 flex items-center justify-center rounded shrink-0
                   opacity-0 group-hover:opacity-100
                   text-[var(--text-secondary)] hover:bg-red-100 dark:hover:bg-red-900/30 hover:text-red-500
                   transition-all text-[14px]"
            aria-label="Remove"
          >×</button>
        {:else}
          <!-- Expanded: status + name + sub-line + actions -->
          <span class="text-[13px] shrink-0 mt-0.5 {statusColor(item.status)}
                       {item.status === 'converting' ? 'animate-spin' : ''}">
            {statusIcon(item.status)}
          </span>

          <div class="flex-1 min-w-0">
            <p class="text-[12px] text-[var(--text-primary)] truncate leading-tight"
               title={item.path}>{item.name}</p>

            {#if item.status === 'converting'}
              <!-- progress shown as overlay fill on row -->
            {:else if item.status === 'error'}
              <div class="mt-0.5">
                <div class="flex items-center gap-1">
                  <p class="text-[11px] text-red-500 truncate flex-1">
                    {item.error?.split('\n')[0] ?? 'Conversion failed'}
                  </p>
                  {#if item.error && item.error.includes('\n')}
                    <button
                      onclick={(e) => { e.stopPropagation(); toggleError(item.id); }}
                      class="shrink-0 text-[10px] text-red-400 hover:text-red-600 transition-colors"
                    >{expandedErrors.has(item.id) ? '▾ Hide' : '▸ Details'}</button>
                  {/if}
                </div>
                {#if expandedErrors.has(item.id)}
                  <pre class="mt-1 text-[11px] text-red-400 font-mono overflow-y-auto
                               max-h-[200px] bg-[var(--surface)] rounded p-1.5 whitespace-pre-wrap
                               break-all">{item.error}</pre>
                {/if}
              </div>
            {:else if item.status === 'done'}
              <p class="text-[11px] text-green-500">Converted</p>
            {:else if item.status === 'cancelled'}
              <p class="text-[11px] text-orange-400">Cancelled</p>
            {:else}
              {@const stats = fileStats(item)}
              {#if stats}
                <p class="text-[11px] text-[var(--text-secondary)] font-mono">{stats}</p>
              {/if}
            {/if}
          </div>

          <div class="flex items-center gap-0.5 shrink-0 opacity-0 group-hover:opacity-100 transition-opacity">
            <button
              onclick={(e) => { e.stopPropagation(); oninfo?.(item); }}
              class="w-5 h-5 flex items-center justify-center rounded
                     text-[var(--text-secondary)] hover:text-[var(--accent)]
                     hover:bg-[var(--surface)] transition-all text-[12px]"
              aria-label="File info"
            >ⓘ</button>
            {#if item.status === 'converting'}
              <button
                onclick={(e) => { e.stopPropagation(); oncancel?.(item.id); }}
                class="w-5 h-5 flex items-center justify-center rounded
                       text-[var(--text-secondary)] hover:text-orange-500
                       hover:bg-orange-50 dark:hover:bg-orange-900/20
                       transition-all text-[13px]"
                aria-label="Cancel"
              >⊘</button>
            {/if}
            <button
              onclick={(e) => { e.stopPropagation(); onremove?.(item.id); }}
              class="w-5 h-5 flex items-center justify-center rounded
                     text-[var(--text-secondary)]
                     hover:bg-red-100 dark:hover:bg-red-900/30 hover:text-red-500
                     transition-all text-[14px]"
              aria-label="Remove"
            >×</button>
          </div>
        {/if}
      </div>
    {/each}
  {/if}
</div>
