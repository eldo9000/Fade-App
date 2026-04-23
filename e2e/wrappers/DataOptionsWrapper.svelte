<script>
  import DataOptions from '../../src/lib/DataOptions.svelte';

  let { initialOptions = {} } = $props();

  let options = $state({ ...initialOptions });
</script>

<DataOptions bind:options />
