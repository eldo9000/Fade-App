<script>
  import ArchiveOptions from '../../src/lib/ArchiveOptions.svelte';

  let { initialOptions = {} } = $props();

  let options = $state({ ...initialOptions });
</script>

<ArchiveOptions bind:options />
