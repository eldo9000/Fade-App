<script>
  import FormatPicker from './FormatPicker.svelte';

  let { options = $bindable() } = $props();
  const formats = ['zip', 'tar', 'gz', '7z'];
</script>

<FormatPicker bind:options {formats} ariaLabel="Archive conversion options" upperCase={false}>
  {#if options.output_format !== 'tar'}
    <fieldset data-tooltip="0 store only (no compression) · 9 max compression (slower)">
      <legend class="text-[12px] font-medium text-[var(--text-secondary)] uppercase tracking-wide mb-2">
        Compression — {options.archive_compression}
      </legend>
      <input type="range" min="0" max="9" step="1" bind:value={options.archive_compression}
        class="fade-range"
        style="--fade-range-pct:{((options.archive_compression ?? 0) / 9) * 100}%" />
      <div class="flex justify-between text-[10px] text-[var(--text-secondary)] mt-1">
        <span>0 store</span><span>9 smallest</span>
      </div>
    </fieldset>
  {/if}
</FormatPicker>
