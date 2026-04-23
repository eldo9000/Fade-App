<script>
  import { seg } from './segStyles.js';

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

<div class="space-y-3" role="form" aria-label={ariaLabel ?? label}>
  <fieldset data-tooltip="Output container/format for every file in the queue">
    <legend class="fade-label">{label}</legend>
    <div class="inline-flex flex-wrap">
      {#each formats as fmt, i}
        <button onclick={() => options.output_format = fmt}
          data-tooltip={`Convert to ${formatted(fmt)}`}
          class={seg(options.output_format === fmt, i, formats.length)}
        >{formatted(fmt)}</button>
      {/each}
    </div>
  </fieldset>
  {@render children?.()}
</div>
