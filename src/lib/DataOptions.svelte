<script>
  import FormatPicker from './FormatPicker.svelte';

  let { options = $bindable() } = $props();
  const formats = ['json', 'csv', 'yaml', 'toml', 'xml'];
</script>

<FormatPicker bind:options {formats} ariaLabel="Data conversion options">
  {#if options.output_format === 'json'}
    <fieldset data-tooltip="Indent JSON with newlines and 2-space indentation for readability — unchecked emits compact single-line JSON">
      <legend class="text-[12px] font-medium text-[var(--text-secondary)] uppercase tracking-wide mb-2">Formatting</legend>
      <label class="flex items-center gap-2 cursor-pointer">
        <input type="checkbox" bind:checked={options.pretty_print} class="accent-[var(--accent)]" />
        <span class="text-[12px] text-[var(--text-primary)]">Pretty print</span>
      </label>
    </fieldset>
  {/if}
  {#if options.output_format === 'csv'}
    <fieldset data-tooltip="Field separator — comma is standard · semicolon common in European locales · tab for TSV · pipe for data with commas">
      <legend class="text-[12px] font-medium text-[var(--text-secondary)] uppercase tracking-wide mb-2">Delimiter</legend>
      <div class="flex gap-2">
        {#each [',', ';', '\t', '|'] as d}
          <button onclick={() => options.csv_delimiter = d}
            class="px-3 py-1 rounded text-[12px] border transition-colors font-mono
              {options.csv_delimiter === d
                ? 'bg-[var(--accent)] text-white border-[var(--accent)]'
                : 'border-[var(--border)] text-[var(--text-primary)] hover:border-[var(--accent)]'}"
          >{d === '\t' ? 'Tab' : d}</button>
        {/each}
      </div>
    </fieldset>
  {/if}
</FormatPicker>
