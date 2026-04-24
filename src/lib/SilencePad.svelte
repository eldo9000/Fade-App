<script>
  let { padFront = $bindable(null), padEnd = $bindable(null) } = $props();

  const MAX = 60;     // seconds
  const CURVE = 2.55; // exponent: higher = slower ramp at start, steeper at end

  // Slider position (0–100) ↔ seconds (0–MAX) mapping
  const posToSecs = (p) => Math.pow(Math.max(0, p) / 100, CURVE) * MAX;
  const secsToPos = (s) => Math.pow(Math.max(0, s) / MAX, 1 / CURVE) * 100;

  // Slider drives `pos`; seconds derived from it
  let frontPos = $state(secsToPos(padFront ?? 0));
  let endPos   = $state(secsToPos(padEnd   ?? 0));

  let frontVal = $derived(+posToSecs(frontPos).toFixed(2));
  let endVal   = $derived(+posToSecs(endPos).toFixed(2));

  // Commit to options — guard treats undefined and null as equivalent so the
  // initial mount (where options.pad_front is undefined) doesn't trigger a
  // spurious reactive cascade that collapses the parent {#if} block.
  $effect(() => {
    const next = frontVal > 0.05 ? frontVal : null;
    if ((padFront ?? null) !== next) padFront = next;
  });
  $effect(() => {
    const next = endVal > 0.05 ? endVal : null;
    if ((padEnd ?? null) !== next) padEnd = next;
  });

  // External reset
  $effect(() => { if (padFront == null && frontPos !== 0) frontPos = 0; });
  $effect(() => { if (padEnd   == null && endPos   !== 0) endPos   = 0; });
</script>

<div class="flex items-center gap-0">
  <!-- Front: inverted slider, thumb starts at right edge -->
  <div class="flex-1 min-w-0 flex flex-col gap-1">
    <input type="range" min="0" max="100" step="0.1"
           bind:value={frontPos}
           aria-label="Extend front silence"
           data-tooltip="Prepend silence before the audio starts. Curve is exponential — fine control near 0, coarser near 60s."
           style="direction:rtl; --fade-range-pct:{frontPos}%; --fade-range-dir:to left"
           class="fade-range" />
    <div class="flex justify-between items-center px-0.5">
      <span class="font-mono text-[11px] text-[var(--text-primary)]">{frontVal.toFixed(1)}s</span>
      <span class="text-[10px] text-[var(--text-secondary)] uppercase tracking-wide">← Front</span>
    </div>
  </div>

  <!-- Divider -->
  <div class="w-px self-stretch bg-[var(--border)] mx-1"></div>

  <!-- End: standard slider, thumb starts at left edge -->
  <div class="flex-1 min-w-0 flex flex-col gap-1">
    <input type="range" min="0" max="100" step="0.1"
           bind:value={endPos}
           aria-label="Extend end silence"
           data-tooltip="Append silence after the audio ends. Curve is exponential — fine control near 0, coarser near 60s."
           style="--fade-range-pct:{endPos}%"
           class="fade-range" />
    <div class="flex justify-between items-center px-0.5">
      <span class="text-[10px] text-[var(--text-secondary)] uppercase tracking-wide">End →</span>
      <span class="font-mono text-[11px] text-[var(--text-primary)]">{endVal.toFixed(1)}s</span>
    </div>
  </div>
</div>
