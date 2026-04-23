<script>
  import VideoOptions from '../../src/lib/VideoOptions.svelte';

  let { initialOptions = {}, errors = {} } = $props();

  let options = $state({ ...initialOptions });
</script>

<VideoOptions bind:options {errors} />
