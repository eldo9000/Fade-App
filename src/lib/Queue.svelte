<script>
  let { queue, onadd, onremove, onclear, oncancel, oninfo } = $props();

  function statusColor(status) {
    switch (status) {
      case 'done':      return 'text-green-500';
      case 'error':     return 'text-red-500';
      case 'cancelled': return 'text-orange-400';
      case 'converting': return 'text-[var(--accent)]';
      default:          return 'text-[var(--text-secondary)]';
    }
  }

  function statusIcon(status) {
    switch (status) {
      case 'done':      return '✓';
      case 'error':     return '✕';
      case 'cancelled': return '⊘';
      case 'converting': return '↻';
      default:          return '·';
    }
  }

  // Per-item expanded error state
  let expandedErrors = $state(new Set());

  function toggleError(id) {
    const next = new Set(expandedErrors);
    if (next.has(id)) next.delete(id);
    else next.add(id);
    expandedErrors = next;
  }

  // Drag-and-drop onto the queue panel itself
  let dropActive = $state(false);

  function onDragover(e) {
    e.preventDefault();
    e.stopPropagation();
    dropActive = true;
  }
  function onDragleave(e) {
    e.stopPropagation();
    dropActive = false;
  }
  function onDrop(e) {
    e.preventDefault();
    e.stopPropagation();
    dropActive = false;
    const paths = Array.from(e.dataTransfer?.files ?? []).map(f => f.path ?? f.name);
    if (paths.length) onadd?.(paths);
  }
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
  class="flex flex-col h-full {dropActive ? 'ring-2 ring-inset ring-[var(--accent)]' : ''}"
  ondragover={onDragover}
  ondragleave={onDragleave}
  ondrop={onDrop}
>

  <!-- Header -->
  <div class="flex items-center justify-between px-3 py-2 border-b border-[var(--border)] shrink-0">
    <span class="text-[12px] font-medium text-[var(--text-secondary)] uppercase tracking-wide">
      Queue ({queue.length})
    </span>
    {#if queue.length > 0}
      <button
        onclick={() => onclear?.()}
        class="text-[11px] text-[var(--text-secondary)] hover:text-red-500 transition-colors"
        aria-label="Clear queue"
      >Clear</button>
    {/if}
  </div>

  <!-- File list -->
  <div class="flex-1 min-h-0 overflow-y-auto" role="list" aria-label="Files in queue">
    {#if queue.length === 0}
      <div class="flex flex-col items-center justify-center h-full gap-2 p-4 text-center">
        <svg width="28" height="28" viewBox="0 0 24 24" fill="none" stroke="currentColor"
             stroke-width="1.5" class="text-[var(--border)]">
          <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"/>
          <polyline points="17 8 12 3 7 8"/>
          <line x1="12" y1="3" x2="12" y2="15"/>
        </svg>
        <p class="text-[12px] text-[var(--text-secondary)] leading-relaxed">
          Drop files here<br>or drag from anywhere
        </p>
      </div>
    {:else}
      {#each queue as item (item.id)}
        <div
          role="listitem"
          class="flex items-start gap-2 px-3 py-2 border-b border-[var(--border)] group"
        >
          <!-- Status icon -->
          <span class="text-[13px] shrink-0 mt-0.5 {statusColor(item.status)}
                       {item.status === 'converting' ? 'animate-spin' : ''}">
            {statusIcon(item.status)}
          </span>

          <!-- File name + progress/error -->
          <div class="flex-1 min-w-0">
            <p class="text-[12px] text-[var(--text-primary)] truncate leading-tight"
               title={item.path}>{item.name}</p>

            {#if item.status === 'converting'}
              <div class="mt-0.5 h-0.5 rounded-full bg-[var(--border)] overflow-hidden">
                <div class="h-full bg-[var(--accent)] transition-all duration-300"
                     style="width: {item.percent}%"></div>
              </div>

            {:else if item.status === 'error'}
              <!-- First line of error + expand toggle -->
              <div class="mt-0.5">
                <div class="flex items-center gap-1">
                  <p class="text-[11px] text-red-500 truncate flex-1">
                    {item.error?.split('\n')[0] ?? 'Conversion failed'}
                  </p>
                  {#if item.error && item.error.includes('\n')}
                    <button
                      onclick={() => toggleError(item.id)}
                      class="shrink-0 text-[10px] text-red-400 hover:text-red-600 transition-colors"
                      aria-label="Toggle error details"
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
              <p class="text-[11px] text-[var(--text-secondary)] capitalize">{item.mediaType}</p>
            {/if}
          </div>

          <!-- Action buttons (hover reveals) -->
          <div class="flex items-center gap-0.5 shrink-0 opacity-0 group-hover:opacity-100 transition-opacity">
            <!-- Info button -->
            <button
              onclick={() => oninfo?.(item)}
              class="w-5 h-5 flex items-center justify-center rounded
                     text-[var(--text-secondary)] hover:text-[var(--accent)]
                     hover:bg-[var(--surface)] transition-all text-[12px]"
              aria-label="File info for {item.name}"
            >ⓘ</button>

            <!-- Cancel button (converting only) -->
            {#if item.status === 'converting'}
              <button
                onclick={() => oncancel?.(item.id)}
                class="w-5 h-5 flex items-center justify-center rounded
                       text-[var(--text-secondary)] hover:text-orange-500
                       hover:bg-orange-50 dark:hover:bg-orange-900/20
                       transition-all text-[13px]"
                aria-label="Cancel {item.name}"
              >⊘</button>
            {/if}

            <!-- Remove button -->
            <button
              onclick={() => onremove?.(item.id)}
              class="w-5 h-5 flex items-center justify-center rounded
                     text-[var(--text-secondary)]
                     hover:bg-red-100 dark:hover:bg-red-900/30 hover:text-red-500
                     transition-all text-[14px]"
              aria-label="Remove {item.name}"
            >×</button>
          </div>
        </div>
      {/each}
    {/if}
  </div>

</div>
