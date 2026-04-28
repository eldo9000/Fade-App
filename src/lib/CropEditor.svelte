<script>
  let {
    imageOptions = $bindable(),
    imgEl,
    imgNaturalW,
    imgNaturalH,
    previewAreaEl,
    cropActive = $bindable(),
    cropAspect = $bindable(),
    selectedId,
  } = $props();

  const CROP_MIN = 0.04;
  let cropRect   = $state({ x: 0.1, y: 0.1, w: 0.8, h: 0.8 });
  let cropDrag   = $state(null);

  // Reset crop when the selected file changes
  $effect(() => { selectedId; cropActive = false; });

  export function activate(aspect) { activateCrop(aspect); }
  export function clear() { clearCropValues(); }
  export function deactivate() { cropActive = false; }

  function getImgBounds() {
    if (!imgEl || !previewAreaEl) return null;
    const ir = imgEl.getBoundingClientRect();
    const pr = previewAreaEl.getBoundingClientRect();
    return { x: ir.left - pr.left, y: ir.top - pr.top, w: ir.width, h: ir.height };
  }

  function initCropRect(aspect) {
    if (!aspect || !imgNaturalW || !imgNaturalH) return { x: 0.1, y: 0.1, w: 0.8, h: 0.8 };
    const nr = aspect * imgNaturalH / imgNaturalW;
    if (nr <= 1) {
      const rw = 0.8, rh = rw / nr;
      if (rh <= 1) return { x: 0.1, y: (1 - rh) / 2, w: rw, h: rh };
      const rh2 = 0.8, rw2 = rh2 * nr;
      return { x: (1 - rw2) / 2, y: 0.1, w: rw2, h: rh2 };
    }
    const rh = 0.8, rw = rh * nr;
    if (rw <= 1) return { x: (1 - rw) / 2, y: 0.1, w: rw, h: rh };
    const rw2 = 0.8, rh2 = rw2 / nr;
    return { x: 0.1, y: (1 - rh2) / 2, w: rw2, h: rh2 };
  }

  function activateCrop(aspect) {
    cropAspect = aspect;
    cropRect = initCropRect(aspect);
    cropActive = true;
  }

  function startCropDrag(e, type) {
    e.stopPropagation();
    const b = getImgBounds();
    if (!b) return;
    cropDrag = { type, sx: e.clientX, sy: e.clientY, r0: { ...cropRect }, b };
  }

  function onCropDragMove(e) {
    if (!cropDrag) return;
    const { type, sx, sy, r0, b } = cropDrag;
    const dx = (e.clientX - sx) / b.w;
    const dy = (e.clientY - sy) / b.h;
    let { x, y, w, h } = r0;

    if (type === 'move') {
      x = Math.max(0, Math.min(1 - w, x + dx));
      y = Math.max(0, Math.min(1 - h, y + dy));
    } else {
      if (type.includes('w')) { const nx = Math.max(0, Math.min(x + w - CROP_MIN, x + dx)); w = x + w - nx; x = nx; }
      if (type.includes('e')) w = Math.max(CROP_MIN, Math.min(1 - x, w + dx));
      if (type.includes('n')) { const ny = Math.max(0, Math.min(y + h - CROP_MIN, y + dy)); h = y + h - ny; y = ny; }
      if (type.includes('s')) h = Math.max(CROP_MIN, Math.min(1 - y, h + dy));
      if (cropAspect && imgNaturalW && imgNaturalH) {
        const nr = cropAspect * imgNaturalH / imgNaturalW;
        if (type.includes('n') || type === 's') { w = Math.min(1 - x, Math.max(CROP_MIN, h * nr)); }
        else { h = Math.min(1 - y, Math.max(CROP_MIN, w / nr)); }
      }
    }
    cropRect = { x, y, w, h };
  }

  function onCropDragEnd() { cropDrag = null; }

  function applyCrop() {
    if (imgNaturalW && imgNaturalH) {
      imageOptions.crop_x = Math.round(cropRect.x * imgNaturalW);
      imageOptions.crop_y = Math.round(cropRect.y * imgNaturalH);
      imageOptions.crop_width = Math.round(cropRect.w * imgNaturalW);
      imageOptions.crop_height = Math.round(cropRect.h * imgNaturalH);
    }
    cropActive = false;
  }

  function cancelCrop() { cropActive = false; }

  function clearCropValues() {
    imageOptions.crop_x = 0; imageOptions.crop_y = 0;
    imageOptions.crop_width = null; imageOptions.crop_height = null;
    cropActive = false;
  }
</script>

<svelte:window
  onmousemove={onCropDragMove}
  onmouseup={onCropDragEnd}
/>

<!-- Crop overlay -->
{#if cropActive && imgEl}
  {@const ib = getImgBounds()}
  {#if ib}
    {@const cx = ib.x + cropRect.x * ib.w}
    {@const cy = ib.y + cropRect.y * ib.h}
    {@const cw = cropRect.w * ib.w}
    {@const ch = cropRect.h * ib.h}
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div class="absolute inset-0 z-30 select-none" style="cursor:default">
      <!-- Dim: top -->
      <div class="absolute bg-black/50 pointer-events-none"
           style="left:{ib.x}px; top:{ib.y}px; width:{ib.w}px; height:{cropRect.y * ib.h}px"></div>
      <!-- Dim: bottom -->
      <div class="absolute bg-black/50 pointer-events-none"
           style="left:{ib.x}px; top:{cy + ch}px; width:{ib.w}px; height:{ib.h - (cropRect.y + cropRect.h) * ib.h}px"></div>
      <!-- Dim: left -->
      <div class="absolute bg-black/50 pointer-events-none"
           style="left:{ib.x}px; top:{cy}px; width:{cropRect.x * ib.w}px; height:{ch}px"></div>
      <!-- Dim: right -->
      <div class="absolute bg-black/50 pointer-events-none"
           style="left:{cx + cw}px; top:{cy}px; width:{ib.w - (cropRect.x + cropRect.w) * ib.w}px; height:{ch}px"></div>

      <!-- Crop border -->
      <div class="absolute pointer-events-none"
           style="left:{cx}px; top:{cy}px; width:{cw}px; height:{ch}px; border:1.5px solid rgba(255,255,255,0.85); box-sizing:border-box">
        <!-- Rule-of-thirds grid -->
        <div class="absolute inset-y-0" style="left:33.33%; border-left:1px solid rgba(255,255,255,0.3)"></div>
        <div class="absolute inset-y-0" style="left:66.66%; border-left:1px solid rgba(255,255,255,0.3)"></div>
        <div class="absolute inset-x-0" style="top:33.33%; border-top:1px solid rgba(255,255,255,0.3)"></div>
        <div class="absolute inset-x-0" style="top:66.66%; border-top:1px solid rgba(255,255,255,0.3)"></div>
      </div>

      <!-- Move area -->
      <!-- svelte-ignore a11y_no_static_element_interactions -->
      <div class="absolute" style="left:{cx}px; top:{cy}px; width:{cw}px; height:{ch}px; cursor:move; z-index:1"
           onmousedown={(e) => startCropDrag(e, 'move')}></div>

      <!-- Corner handles -->
      {#each [['nw',cx,cy,'nwse-resize'],['ne',cx+cw,cy,'nesw-resize'],['sw',cx,cy+ch,'nesw-resize'],['se',cx+cw,cy+ch,'nwse-resize']] as [type,hx,hy,cur]}
        <!-- svelte-ignore a11y_no_static_element_interactions -->
        <div class="absolute w-3 h-3 bg-white rounded-sm shadow"
             style="left:{hx}px; top:{hy}px; transform:translate(-50%,-50%); cursor:{cur}; z-index:3"
             onmousedown={(e) => startCropDrag(e, type)}></div>
      {/each}

      <!-- Edge handles -->
      {#each [['n',cx+cw/2,cy,'ns-resize'],['s',cx+cw/2,cy+ch,'ns-resize'],['w',cx,cy+ch/2,'ew-resize'],['e',cx+cw,cy+ch/2,'ew-resize']] as [type,hx,hy,cur]}
        <!-- svelte-ignore a11y_no_static_element_interactions -->
        <div class="absolute w-3 h-3 bg-white rounded-sm shadow"
             style="left:{hx}px; top:{hy}px; transform:translate(-50%,-50%); cursor:{cur}; z-index:3"
             onmousedown={(e) => startCropDrag(e, type)}></div>
      {/each}

      <!-- Apply / Cancel — always visible inside the overlay -->
      <div class="absolute bottom-3 right-3 flex gap-2" style="z-index:10">
        <button onclick={cancelCrop}
                class="px-3 py-1 rounded bg-black/60 border border-white/15 text-white text-[11px] hover:bg-black/80 transition-colors">
          Cancel
        </button>
        <button onclick={applyCrop}
                class="px-3 py-1 rounded bg-[var(--accent)] text-white text-[11px] font-medium hover:opacity-90 transition-opacity">
          Apply
        </button>
      </div>
    </div>
  {/if}
{/if}
