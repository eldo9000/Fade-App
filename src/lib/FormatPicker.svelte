<script>
  let {
    options = $bindable(),
    formats,
    label = 'Output Format',
    ariaLabel,
    upperCase = true,
    children,
  } = $props();

  const formatted = (fmt) => (upperCase ? fmt.toUpperCase() : fmt);
</script>

<div class="space-y-5" role="form" aria-label={ariaLabel ?? label}>
  <fieldset data-tooltip="Output container/format for every file in the queue">
    <legend class="text-[12px] font-medium text-[var(--text-secondary)] uppercase tracking-wide mb-2">{label}</legend>
    <div class="flex flex-wrap gap-2">
      {#each formats as fmt}
        <button onclick={() => options.output_format = fmt}
          data-tooltip={`Convert to ${formatted(fmt)}`}
          class="px-3 py-1 rounded text-[12px] font-medium border transition-colors
            {options.output_format === fmt
              ? 'bg-[var(--accent)] text-white border-[var(--accent)]'
              : 'border-[var(--border)] text-[var(--text-primary)] hover:border-[var(--accent)]'}"
        >{formatted(fmt)}</button>
      {/each}
    </div>
  </fieldset>
  {@render children?.()}
</div>
