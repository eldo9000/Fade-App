<script>
  import ImageOptions from '../../src/lib/ImageOptions.svelte';

  let {
    initialOptions = {},
    cropActive = false,
    cropAspect = null,
    onqualitystart = null,
    onqualityinput = null,
    onqualityend = null,
    oncropstart = null,
    oncropclear = null,
  } = $props();

  let options = $state({ ...initialOptions });
</script>

<ImageOptions
  bind:options
  {cropActive}
  {cropAspect}
  {onqualitystart}
  {onqualityinput}
  {onqualityend}
  {oncropstart}
  {oncropclear}
/>
