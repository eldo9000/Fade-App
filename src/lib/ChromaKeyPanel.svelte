<script>
  import { invoke, convertFileSrc } from '@tauri-apps/api/core';
  import { markConverting, markError } from './itemStatus.js';

  let {
    selectedItem = $bindable(null),
    videoOptions,
    outputDir,
    outputSeparator,
    setStatus,
  } = $props();

  // ── State ─────────────────────────────────────────────────────────────────
  let chromaAlgo         = $state('chromakey');  // 'chromakey' · 'colorkey' · 'hsvkey'
  let chromaColor        = $state('#00ff00');    // HTML color picker (RGB hex)
  let chromaSimilarity   = $state(0.10);         // 0.01..0.40
  let chromaBlend        = $state(0.10);         // 0.0..0.5 (soft edge)
  let chromaDespill      = $state(true);         // toggle — ignored for colorkey
  let chromaDespillMix   = $state(0.50);         // 0.0..1.0
  let chromaUpsample     = $state(true);         // prepend format=yuv444p
  let chromaOutputFormat = $state('mov_prores4444'); // alpha container
  let chromaPreviewUrl   = $state(null);         // asset:// URL of last preview
  let chromaPreviewLoading = $state(false);
  let chromaPreviewError = $state(null);
  let _chromaPreviewTimer = null;
  let _chromaPreviewKey  = null;                 // last-rendered param hash

  // Cleanup debounce timer on component teardown
  $effect(() => {
    return () => clearTimeout(_chromaPreviewTimer);
  });

  // ── Helpers ───────────────────────────────────────────────────────────────

  function expectedOutputPath(item, newExt, suffix, outputDirOverride, sep = '_') {
    const lastSlash = item.path.lastIndexOf('/');
    const parentDir = lastSlash >= 0 ? item.path.slice(0, lastSlash) : '.';
    const dir = outputDirOverride ?? parentDir;
    const stem = item.ext ? item.name.slice(0, -(item.ext.length + 1)) : item.name;
    return suffix
      ? `${dir}/${stem}${sep}${suffix}.${newExt}`
      : `${dir}/${stem}.${newExt}`;
  }

  function _chromaOutputMeta() {
    switch (chromaOutputFormat) {
      case 'mov_qtrle':        return { ext: 'mov', target: 'mov_qtrle',       suffix: 'keyed' };
      case 'mov_prores4444':   return { ext: 'mov', target: 'mov_prores4444',  suffix: 'keyed' };
      case 'webm_vp9':         return { ext: 'webm',target: 'webm_vp9',        suffix: 'keyed' };
      case 'mkv_ffv1':         return { ext: 'mkv', target: 'mkv_ffv1',        suffix: 'keyed' };
      case 'png_sequence':     return { ext: '',    target: 'png_sequence',    suffix: 'frames' };
      default:                 return { ext: 'mov', target: 'mov_prores4444',  suffix: 'keyed' };
    }
  }

  function _chromaPreviewKeyOf() {
    return [
      selectedItem?.path, chromaAlgo, chromaColor,
      chromaSimilarity, chromaBlend,
      chromaDespill, chromaDespillMix, chromaUpsample,
      videoOptions?.trim_start ?? 0,
    ].join('|');
  }

  async function generateChromaPreview() {
    if (!selectedItem || selectedItem.mediaType !== 'video') return;
    const key = _chromaPreviewKeyOf();
    if (key === _chromaPreviewKey && chromaPreviewUrl) return;
    if (_chromaPreviewTimer) clearTimeout(_chromaPreviewTimer);
    _chromaPreviewTimer = setTimeout(async () => {
      chromaPreviewLoading = true;
      chromaPreviewError = null;
      try {
        const t = Number(videoOptions?.trim_start) > 0
          ? Number(videoOptions.trim_start)
          : 1.0;
        const path = await invoke('chroma_key_preview', {
          inputPath: selectedItem.path,
          timeS: t,
          algo: chromaAlgo,
          colorHex: String(chromaColor),
          similarity: Number(chromaSimilarity),
          blend: Number(chromaBlend),
          despill: !!chromaDespill,
          despillMix: Number(chromaDespillMix),
          upsample: !!chromaUpsample,
        });
        // Cache-bust so the <img> refreshes when the same path is rewritten.
        chromaPreviewUrl = convertFileSrc(path) + '?t=' + Date.now();
        _chromaPreviewKey = key;
      } catch (err) {
        chromaPreviewError = String(err);
      } finally {
        chromaPreviewLoading = false;
      }
    }, 250);
  }

  async function runChromaKey() {
    if (!selectedItem) { setStatus('Select a file first', 'error'); return; }
    if (selectedItem.mediaType !== 'video') { setStatus('Chroma key: video file required', 'error'); return; }
    if (selectedItem.status === 'converting') return;

    const meta = _chromaOutputMeta();
    let outPath;
    if (meta.target === 'png_sequence') {
      const base = expectedOutputPath(selectedItem, 'x', meta.suffix, outputDir, outputSeparator);
      outPath = base.replace(/\.x$/, '');
    } else {
      outPath = expectedOutputPath(selectedItem, meta.ext, meta.suffix, outputDir, outputSeparator);
    }

    markConverting(selectedItem);
    try {
      await invoke('run_operation', {
        jobId: selectedItem.id,
        operation: {
          type: 'chroma_key',
          input_path: selectedItem.path,
          output_path: outPath,
          algo: chromaAlgo,
          color_hex: String(chromaColor),
          similarity: Number(chromaSimilarity),
          blend: Number(chromaBlend),
          despill: !!chromaDespill,
          despill_mix: Number(chromaDespillMix),
          upsample: !!chromaUpsample,
          output_target: meta.target,
          trim_start: videoOptions?.trim_start ?? null,
          trim_end:   videoOptions?.trim_end   ?? null,
        },
      });
    } catch (err) {
      markError(selectedItem, err);
      setStatus(`Chroma key failed: ${err}`, 'error');
    }
  }
</script>

<!-- ── Controls ──────────────────────────────────────────────────────────── -->

<!-- Row 1: algorithm segmented -->
<div class="flex flex-wrap items-center gap-2 w-full">
  <div class="inline-flex items-center rounded-md overflow-hidden border border-[var(--border)]">
    <span class="px-2 text-[10px] uppercase tracking-wider text-white/40 font-semibold">Algo</span>
    <div class="w-px h-6 bg-[var(--border)]"></div>
    <button onclick={() => { chromaAlgo = 'chromakey'; generateChromaPreview(); }}
            title="YUV-space key. Best for green / blue screens with evenly-lit backdrops."
            class="px-2.5 py-1.5 text-[11px] font-semibold transition-colors
                   {chromaAlgo === 'chromakey' ? 'bg-[var(--accent)] text-white' : 'text-white/60 hover:bg-white/5'}">chromakey</button>
    <div class="w-px h-6 bg-[var(--border)]"></div>
    <button onclick={() => { chromaAlgo = 'colorkey'; generateChromaPreview(); }}
            title="RGB-space hard cut. Best for solid flat-colour mattes (title cards, generated BG)."
            class="px-2.5 py-1.5 text-[11px] font-semibold transition-colors
                   {chromaAlgo === 'colorkey' ? 'bg-[var(--accent)] text-white' : 'text-white/60 hover:bg-white/5'}">colorkey</button>
    <div class="w-px h-6 bg-[var(--border)]"></div>
    <button onclick={() => { chromaAlgo = 'hsvkey'; generateChromaPreview(); }}
            title="HSV-space key. Use when lighting on the screen is uneven."
            class="px-2.5 py-1.5 text-[11px] font-semibold transition-colors
                   {chromaAlgo === 'hsvkey' ? 'bg-[var(--accent)] text-white' : 'text-white/60 hover:bg-white/5'}">hsvkey</button>
  </div>
  <label class="flex items-center gap-1.5 rounded border border-[var(--border)] px-2 py-1 cursor-pointer"
         title="Key colour. Defaults to pure green. Use the eyedropper-style native picker.">
    <span class="text-[10px] uppercase tracking-wider text-white/40 font-semibold">Colour</span>
    <input type="color" bind:value={chromaColor}
           oninput={generateChromaPreview}
           class="w-8 h-6 rounded border border-[var(--border)] bg-transparent cursor-pointer"/>
    <span class="text-[11px] text-white/50 font-mono tabular-nums">{chromaColor}</span>
  </label>
</div>
<!-- Row 2: similarity slider -->
<div class="flex items-center gap-2 w-full">
  <span class="text-[10px] uppercase tracking-wider text-white/40 font-semibold shrink-0 w-20">Similarity</span>
  <input type="range" min="0.01" max="0.40" step="0.01"
         bind:value={chromaSimilarity}
         oninput={generateChromaPreview}
         class="flex-1 accent-[var(--accent)]"/>
  <span class="text-[11px] text-white/70 font-mono tabular-nums w-10 text-right">{Number(chromaSimilarity).toFixed(2)}</span>
</div>
<!-- Row 3: blend slider -->
<div class="flex items-center gap-2 w-full">
  <span class="text-[10px] uppercase tracking-wider text-white/40 font-semibold shrink-0 w-20">Blend</span>
  <input type="range" min="0.00" max="0.50" step="0.01"
         bind:value={chromaBlend}
         oninput={generateChromaPreview}
         class="flex-1 accent-[var(--accent)]"/>
  <span class="text-[11px] text-white/70 font-mono tabular-nums w-10 text-right">{Number(chromaBlend).toFixed(2)}</span>
</div>
<!-- Row 4: despill -->
<div class="flex items-center gap-2 w-full"
     title={chromaAlgo === 'colorkey' ? 'Despill not meaningful for hard colorkey cuts.' : 'Remove coloured light bouncing onto the subject from the screen.'}>
  <label class="flex items-center gap-1.5 rounded border border-[var(--border)] px-2 py-1 cursor-pointer shrink-0"
         class:opacity-40={chromaAlgo === 'colorkey'}>
    <input type="checkbox" bind:checked={chromaDespill}
           disabled={chromaAlgo === 'colorkey'}
           onchange={generateChromaPreview}
           class="accent-[var(--accent)]"/>
    <span class="text-[11px] text-white/70 font-medium">Despill</span>
  </label>
  <span class="text-[10px] uppercase tracking-wider text-white/40 font-semibold shrink-0 w-8">Mix</span>
  <input type="range" min="0.00" max="1.00" step="0.05"
         bind:value={chromaDespillMix}
         oninput={generateChromaPreview}
         disabled={!chromaDespill || chromaAlgo === 'colorkey'}
         class="flex-1 accent-[var(--accent)] disabled:opacity-40"/>
  <span class="text-[11px] text-white/70 font-mono tabular-nums w-10 text-right">{Number(chromaDespillMix).toFixed(2)}</span>
</div>
<!-- Row 5: upsample toggle -->
<div class="flex items-center gap-2 w-full">
  <label class="flex items-center gap-1.5 rounded border border-[var(--border)] px-2 py-1 cursor-pointer"
         title="Prepend format=yuv444p before the key. Helps noticeably on 4:2:0 sources where the chroma planes are blurry.">
    <input type="checkbox" bind:checked={chromaUpsample}
           onchange={generateChromaPreview}
           class="accent-[var(--accent)]"/>
    <span class="text-[11px] text-white/70 font-medium">Upsample chroma (yuv444p)</span>
  </label>
</div>
<!-- Row 6: output container segmented -->
<div class="flex flex-wrap items-center gap-2 w-full">
  <div class="inline-flex items-center rounded-md overflow-hidden border border-[var(--border)]">
    <span class="px-2 text-[10px] uppercase tracking-wider text-white/40 font-semibold">Output</span>
    <div class="w-px h-6 bg-[var(--border)]"></div>
    <button onclick={() => chromaOutputFormat = 'mov_prores4444'}
            title="MOV + ProRes 4444 — editorial standard, great alpha, big files."
            class="px-2.5 py-1.5 text-[11px] font-semibold transition-colors
                   {chromaOutputFormat === 'mov_prores4444' ? 'bg-[var(--accent)] text-white' : 'text-white/60 hover:bg-white/5'}">ProRes 4444</button>
    <div class="w-px h-6 bg-[var(--border)]"></div>
    <button onclick={() => chromaOutputFormat = 'mov_qtrle'}
            title="MOV + QuickTime Animation (RLE) — lossless, huge files, very compatible."
            class="px-2.5 py-1.5 text-[11px] font-semibold transition-colors
                   {chromaOutputFormat === 'mov_qtrle' ? 'bg-[var(--accent)] text-white' : 'text-white/60 hover:bg-white/5'}">QtRLE</button>
    <div class="w-px h-6 bg-[var(--border)]"></div>
    <button onclick={() => chromaOutputFormat = 'webm_vp9'}
            title="WebM + VP9 with yuva420p — browser-playable alpha video."
            class="px-2.5 py-1.5 text-[11px] font-semibold transition-colors
                   {chromaOutputFormat === 'webm_vp9' ? 'bg-[var(--accent)] text-white' : 'text-white/60 hover:bg-white/5'}">VP9+alpha</button>
    <div class="w-px h-6 bg-[var(--border)]"></div>
    <button onclick={() => chromaOutputFormat = 'png_sequence'}
            title="PNG sequence — writes a sibling &lt;name&gt;_frames/ directory, one file per frame."
            class="px-2.5 py-1.5 text-[11px] font-semibold transition-colors
                   {chromaOutputFormat === 'png_sequence' ? 'bg-[var(--accent)] text-white' : 'text-white/60 hover:bg-white/5'}">PNG seq</button>
    <div class="w-px h-6 bg-[var(--border)]"></div>
    <button onclick={() => chromaOutputFormat = 'mkv_ffv1'}
            title="MKV + FFV1 — lossless archival, smaller than QtRLE."
            class="px-2.5 py-1.5 text-[11px] font-semibold transition-colors
                   {chromaOutputFormat === 'mkv_ffv1' ? 'bg-[var(--accent)] text-white' : 'text-white/60 hover:bg-white/5'}">FFV1</button>
  </div>
</div>
<!-- Row 7: preview + run -->
<div class="flex items-center gap-2 w-full">
  <button
    onclick={generateChromaPreview}
    disabled={!selectedItem || selectedItem.mediaType !== 'video'}
    class="px-3 py-1.5 rounded text-[12px] font-semibold border border-[var(--accent)] text-[var(--accent)] hover:bg-[color-mix(in_srgb,var(--accent)_12%,transparent)] transition-colors disabled:opacity-40"
  >{chromaPreviewLoading ? 'Previewing…' : 'Preview frame'}</button>
  <button
    onclick={runChromaKey}
    disabled={!selectedItem || selectedItem.mediaType !== 'video' || selectedItem.status === 'converting'}
    class="px-3 py-1.5 rounded text-[12px] font-semibold bg-[var(--accent)] text-white hover:opacity-90 transition-opacity disabled:opacity-40"
  >Run Chroma Key</button>
</div>
<!-- Preview image area with checkerboard backdrop. -->
<div class="w-full rounded border border-[var(--border)] overflow-hidden"
     style="background-image:
              linear-gradient(45deg, #333 25%, transparent 25%),
              linear-gradient(-45deg, #333 25%, transparent 25%),
              linear-gradient(45deg, transparent 75%, #333 75%),
              linear-gradient(-45deg, transparent 75%, #333 75%);
            background-size: 16px 16px;
            background-position: 0 0, 0 8px, 8px -8px, -8px 0;
            background-color: #1a1a1a;
            min-height: 120px;">
  {#if chromaPreviewUrl}
    <img src={chromaPreviewUrl} alt="Chroma key preview"
         class="w-full block"/>
  {:else if chromaPreviewError}
    <div class="p-3 text-[11px] text-red-400 font-mono">{chromaPreviewError}</div>
  {:else}
    <div class="p-3 text-[11px] text-white/30">No preview yet — click Preview frame after selecting a video.</div>
  {/if}
</div>
