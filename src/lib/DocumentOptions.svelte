<script>
  let { options = $bindable() } = $props();
  const formats = ['html', 'md', 'txt', 'pdf', 'docx'];
</script>

<div class="space-y-5" role="form" aria-label="Document conversion options">
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
</div>
