<script>
  import { convertFileSrc } from '@tauri-apps/api/core';

  let { queue, selectedId, onselect, onremove, oncancel, compatibleTypes = [], compact = false } = $props();

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
      default:           return '';
    }
  }

  let expandedErrors = $state(new Set());

  function toggleError(id) {
    const next = new Set(expandedErrors);
    if (next.has(id)) next.delete(id);
    else next.add(id);
    expandedErrors = next;
  }

  // ── Info hover popover ─────────────────────────────────────────────────────
  let hoveredItem  = $state(null);
  let popoverLeft  = $state(0);
  let popoverTop   = $state(0);
  let copiedField  = $state(null);
  let _hideTimer   = null;

  function _scheduleHide() {
    clearTimeout(_hideTimer);
    _hideTimer = setTimeout(() => { hoveredItem = null; }, 120);
  }
  function _cancelHide() { clearTimeout(_hideTimer); }

  function onItemEnter(e, item) {
    _cancelHide();
    hoveredItem = item;
    const r = e.currentTarget.getBoundingClientRect();
    popoverTop  = r.top + r.height / 2;
    popoverLeft = r.right + 1;
  }
  function onItemLeave()    { _scheduleHide(); }
  function onPopoverEnter() { _cancelHide(); }
  function onPopoverLeave() { _scheduleHide(); }

  function copyValue(field, value) {
    navigator.clipboard.writeText(String(value)).catch(() => {});
    copiedField = field;
    clearTimeout(_hideTimer);
    setTimeout(() => { copiedField = null; }, 1500);
  }

  function fmtSize(b) {
    if (!b) return null;
    if (b < 1024)       return `${b} B`;
    if (b < 1048576)    return `${(b / 1024).toFixed(1)} KB`;
    if (b < 1073741824) return `${(b / 1048576).toFixed(1)} MB`;
    return `${(b / 1073741824).toFixed(2)} GB`;
  }
  function fmtDur(s) {
    if (!s) return null;
    const h = Math.floor(s / 3600), m = Math.floor((s % 3600) / 60), sec = Math.floor(s % 60);
    return h > 0
      ? `${h}:${String(m).padStart(2,'0')}:${String(sec).padStart(2,'0')}`
      : `${m}:${String(sec).padStart(2,'0')}`;
  }
</script>

<!-- File list -->
<!-- svelte-ignore a11y_click_events_have_key_events -->
<!-- svelte-ignore a11y_no_static_element_interactions -->
<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
<div class="flex-1 min-h-0 overflow-y-auto" role="list" aria-label="Files in queue"
     onclick={(e) => { if (e.target === e.currentTarget) onselect?.(null); }}>
  {#if queue.length === 0}
    <div class="h-full flex flex-col items-center justify-center gap-2 px-6 text-center select-none">
      <svg width="22" height="22" viewBox="0 0 24 24" fill="none" stroke="currentColor"
           stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"
           class="text-[var(--border)]">
        <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"/>
        <polyline points="17 8 12 3 7 8"/>
        <line x1="12" y1="3" x2="12" y2="15"/>
      </svg>
      <p class="text-[11px] text-[var(--text-secondary)]">Drop files or click Browse</p>
    </div>
  {:else}
    {#each queue as item (item.id)}
      <!-- svelte-ignore a11y_click_events_have_key_events -->
      <!-- svelte-ignore a11y_no_static_element_interactions -->
      <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
      <div
        role="listitem"
        onclick={() => onselect?.(selectedId === item.id ? null : item.id)}
        onmouseenter={(e) => onItemEnter(e, item)}
        onmouseleave={onItemLeave}
        class="relative overflow-hidden flex items-center gap-2 px-3 {compact ? 'py-1' : 'py-2'} border-b border-[var(--border)] group cursor-pointer transition-colors
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
          <!-- Expanded: thumbnail + status icon + name + actions -->

          <!-- Thumbnail -->
          <div class="shrink-0 w-9 h-9 rounded overflow-hidden bg-[var(--surface)] flex items-center justify-center">
            {#if item.mediaType === 'image'}
              <img src={convertFileSrc(item.path)} alt="" class="w-full h-full object-cover" />
            {:else if item.mediaType === 'video'}
              <!-- film icon -->
              <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5"
                   stroke-linecap="round" stroke-linejoin="round" class="text-[var(--text-secondary)]">
                <rect x="2" y="2" width="20" height="20" rx="2.18"/>
                <line x1="7" y1="2" x2="7" y2="22"/><line x1="17" y1="2" x2="17" y2="22"/>
                <line x1="2" y1="12" x2="22" y2="12"/><line x1="2" y1="7" x2="7" y2="7"/>
                <line x1="2" y1="17" x2="7" y2="17"/><line x1="17" y1="17" x2="22" y2="17"/>
                <line x1="17" y1="7" x2="22" y2="7"/>
              </svg>
            {:else}
              <!-- audio wave icon -->
              <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5"
                   stroke-linecap="round" stroke-linejoin="round" class="text-[var(--text-secondary)]">
                <path d="M9 18V5l12-2v13"/><circle cx="6" cy="18" r="3"/><circle cx="18" cy="16" r="3"/>
              </svg>
            {/if}
          </div>

          {#if statusIcon(item.status)}
            <span class="text-[13px] shrink-0 {statusColor(item.status)}
                         {item.status === 'converting' ? 'animate-spin' : ''}">
              {statusIcon(item.status)}
            </span>
          {/if}

          <div class="flex-1 min-w-0">
            <p class="text-[15px] font-medium text-[var(--text-primary)] truncate leading-tight"
               title={item.path}>{item.name}</p>

            {#if item.status === 'error'}
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
            {/if}
          </div>

          <div class="flex items-center gap-0.5 shrink-0 opacity-0 group-hover:opacity-100 transition-opacity">
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

<!-- Info hover popover — fixed so it escapes the sidebar's overflow -->
{#if hoveredItem}
  {@const info = hoveredItem.info}
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div
    onmouseenter={onPopoverEnter}
    onmouseleave={onPopoverLeave}
    style="position:fixed; left:{popoverLeft}px; top:{popoverTop}px; transform:translateY(-50%); z-index:1000"
  >
    <!-- Box -->
    <div style="background:#1e1e22; border:1px solid rgba(255,255,255,0.1); border-radius:7px;
                padding:10px 13px; min-width:180px; max-width:248px;
                box-shadow:0 8px 24px rgba(0,0,0,0.5)">
      <!-- Filename -->
      <p style="font-size:11px; font-weight:600; color:rgba(255,255,255,0.92);
                margin-bottom:8px; white-space:nowrap; overflow:hidden; text-overflow:ellipsis;
                max-width:222px"
         title={hoveredItem.path}>{hoveredItem.name}</p>
      <!-- Info rows -->
      <div style="display:flex; flex-direction:column; gap:4px">
        {#if info}
          {#each [
            (info.format || info.media_type) ? { key:'type',       label:'Type',       val: (info.format ?? info.media_type).toUpperCase() } : null,
            info.codec                       ? { key:'codec',      label:'Codec',      val: info.codec }                                      : null,
            (info.width && info.height)      ? { key:'res',        label:'Resolution', val: `${info.width}×${info.height}` }                  : null,
            info.duration_secs               ? { key:'dur',        label:'Duration',   val: fmtDur(info.duration_secs) }                      : null,
            info.file_size                   ? { key:'size',       label:'Size',       val: fmtSize(info.file_size) }                         : null,
          ].filter(Boolean) as row}
            <!-- svelte-ignore a11y_no_static_element_interactions -->
            <div style="display:flex; align-items:center; justify-content:space-between; gap:10px; border-radius:3px; padding:1px 2px"
                 class="group/row">
              <!-- Copy button -->
              <button
                onclick={() => copyValue(row.key, row.val)}
                style="flex-shrink:0; width:14px; height:14px; display:flex; align-items:center; justify-content:center;
                       background:none; border:none; padding:0; cursor:pointer;
                       color:{copiedField === row.key ? 'rgba(96,165,250,1)' : 'rgba(255,255,255,0.25)'}; transition:color 0.12s"
                title="Copy"
              >
                {#if copiedField === row.key}
                  <!-- Check -->
                  <svg width="9" height="9" viewBox="0 0 12 12" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round">
                    <polyline points="2,6 5,9 10,3"/>
                  </svg>
                {:else}
                  <!-- Clipboard -->
                  <svg width="9" height="9" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round">
                    <rect x="9" y="9" width="13" height="13" rx="2"/>
                    <path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1"/>
                  </svg>
                {/if}
              </button>
              <span style="font-size:10px; color:rgba(255,255,255,0.38); flex-shrink:0">{row.label}</span>
              <span style="font-size:10px; color:rgba(255,255,255,0.78); font-family:monospace; margin-left:auto">{row.val}</span>
            </div>
          {/each}
        {:else}
          <span style="font-size:10px; color:rgba(255,255,255,0.3)">Loading…</span>
        {/if}
      </div>
    </div>
  </div>
{/if}
