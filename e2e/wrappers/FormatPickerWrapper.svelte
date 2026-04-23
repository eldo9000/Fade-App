<script>
  import FormatPicker from '../../src/lib/FormatPicker.svelte';

  let {
    formats,
    initialFormat = 'jpg',
    label = 'Output Format',
    upperCase = true,
    ariaLabel = undefined,
  } = $props();

  let options = $state({ output_format: initialFormat });
</script>

<FormatPicker bind:options {formats} {label} {upperCase} {ariaLabel} />
