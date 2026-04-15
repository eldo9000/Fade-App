<script>
  let { options = $bindable() } = $props();
  const formats = ['json', 'csv', 'yaml', 'toml', 'xml'];
</script>

<div class="space-y-5" role="form" aria-label="Data conversion options">
  <fieldset>
    <legend class="text-[12px] font-medium text-[var(--text-secondary)] uppercase tracking-wide mb-2">Output Format</legend>
    <div class="flex flex-wrap gap-2">
      {#each formats as fmt}
        <button onclick={() => options.output_format = fmt}
          class="px-3 py-1 rounded text-[12px] font-medium border transition-colors
            {options.output_format === fmt
              ? 'bg-[var(--accent)] text-white border-[var(--accent)]'
              : 'border-[var(--border)] text-[var(--text-primary)] hover:border-[var(--accent)]'}"
        >{fmt.toUpperCase()}</button>
      {/each}
    </div>
  </fieldset>
  {#if options.output_format === 'json'}
    <fieldset>
      <legend class="text-[12px] font-medium text-[var(--text-secondary)] uppercase tracking-wide mb-2">Formatting</legend>
      <label class="flex items-center gap-2 cursor-pointer">
        <input type="checkbox" bind:checked={options.pretty_print} class="accent-[var(--accent)]" />
        <span class="text-[12px] text-[var(--text-primary)]">Pretty print</span>
      </label>
    </fieldset>
  {/if}
  {#if options.output_format === 'csv'}
    <fieldset>
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
</div>
