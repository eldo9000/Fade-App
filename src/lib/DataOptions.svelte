<script>
  import { seg } from './segStyles.js';

  let { options = $bindable() } = $props();

  const delimiters = [',', ';', '\t', '|'];
</script>

<div class="space-y-3" role="form" aria-label="Data conversion options">
  {#if options.output_format === 'json'}
    <fieldset data-tooltip="Indent JSON with newlines and 2-space indentation for readability — unchecked emits compact single-line JSON">
      <legend class="fade-label">Formatting</legend>
      <label class="inline-flex items-center gap-2.5 cursor-pointer text-[13px]
                    bg-[var(--surface-hint)] border border-[var(--border)] rounded-md px-3 py-2
                    {options.pretty_print ? 'text-[var(--text-primary)]' : 'text-white/75'}">
        <input type="checkbox" bind:checked={options.pretty_print} class="fade-check" />
        Pretty print
      </label>
    </fieldset>
  {:else if options.output_format === 'csv'}
    <fieldset data-tooltip="Field separator — comma is standard · semicolon common in European locales · tab for TSV · pipe for data with commas">
      <legend class="fade-label">Delimiter</legend>
      <div class="grid" style="grid-template-columns:repeat({delimiters.length},1fr)">
        {#each delimiters as d, i}
          <button onclick={() => options.csv_delimiter = d}
            class={seg(options.csv_delimiter === d, i, delimiters.length) + ' font-mono'}
          >{d === '\t' ? 'Tab' : d}</button>
        {/each}
      </div>
    </fieldset>
  {:else}
    <p class="text-[11px] text-[var(--text-secondary)]" data-tooltip="No additional options for this format">No additional options.</p>
  {/if}
</div>
