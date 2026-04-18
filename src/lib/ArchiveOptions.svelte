<script>
  import FormatPicker from './FormatPicker.svelte';

  let { options = $bindable() } = $props();
  const formats = ['zip', 'tar', 'gz', '7z'];

  function seg(active, i, total) {
    const base  = 'px-3 py-1.5 text-center text-[12px] font-medium border transition-colors relative';
    const round = i === 0 ? 'rounded-l-md' : i === total - 1 ? 'rounded-r-md' : '';
    const ml    = i > 0 ? '-ml-px' : '';
    const color = active
      ? 'bg-[var(--accent)] text-white border-[var(--accent)] z-10'
      : 'border-[var(--border)] text-[var(--text-primary)] hover:z-10 hover:border-[var(--accent)] hover:text-[var(--accent)]';
    return [base, round, ml, color].filter(Boolean).join(' ');
  }
</script>

<FormatPicker bind:options {formats} ariaLabel="Archive conversion options" upperCase={false}>
  {#if options.output_format !== 'tar'}
    <fieldset data-tooltip="0 store only (no compression) · 9 max compression (slower)">
      <legend class="text-[12px] font-medium text-[var(--text-secondary)] uppercase tracking-wide mb-2">
        Compression — {options.archive_compression}
      </legend>
      <input type="range" min="0" max="9" step="1" bind:value={options.archive_compression}
        class="w-full accent-[var(--accent)]" />
      <div class="flex justify-between text-[10px] text-[var(--text-secondary)] mt-1">
        <span>0 store</span><span>9 smallest</span>
      </div>
    </fieldset>
  {/if}
</FormatPicker>
