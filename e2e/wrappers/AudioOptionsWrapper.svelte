<script>
  import AudioOptions from '../../src/lib/AudioOptions.svelte';

  let { initialOptions = {}, errors = {} } = $props();

  let options = $state({ ...initialOptions });
</script>

<AudioOptions bind:options {errors} />
