<script>
  import { invoke } from '@tauri-apps/api/core';
  import { listen } from '@tauri-apps/api/event';
  import { markConverting, markError } from './itemStatus.js';

  let {
    selectedItem = $bindable(null),   // $bindable for runAudioNorm which mutates status
    selectedOperation,
    outputDir,
    outputSeparator,
    pickAuxFile,
    setStatus,
  } = $props();

  // ── Per-analyzer state ────────────────────────────────────────────────────

  // Loudness & TP
  let loudnessTarget       = $state('-23');
  let loudnessTargetCustom = $state(-23);
  let loudnessTruePeak     = $state(true);
  let loudnessResult       = $state(null);

  // Audio Norm
  let audioNormMode      = $state('ebu');
  let audioNormTargetI   = $state(-16);
  let audioNormTargetTP  = $state(-1.5);
  let audioNormTargetLRA = $state(11);
  let audioNormLinear    = $state(true);

  // Cut Detection
  let cutDetectAlgo       = $state('scdet');
  let cutDetectThreshold  = $state(10);
  let cutDetectMinShotS   = $state(0.5);
  let cutDetectResults    = $state([]);

  // Black Detection
  let blackDetectMinDur    = $state(0.1);
  let blackDetectPixTh     = $state(0.10);
  let blackDetectPicTh     = $state(0.98);
  let blackDetectResults   = $state([]);

  // VMAF
  let vmafReferencePath = $state(null);
  let vmafDistortedPath = $state(null);
  let vmafModel         = $state('hd');
  let vmafSubsample     = $state(1);
  let vmafResult        = $state(null);

  // FrameMD5
  let frameMd5Stream   = $state('video');
  let frameMd5DiffPath = $state(null);
  let frameMd5Result   = $state(null);

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

  // ── Job-based analysis helper ─────────────────────────────────────────────
  // Invokes a long-running analysis command that spawns a backend thread,
  // registers a cancellable FFmpeg child, and emits its result on
  // `analysis-result:{jobId}`. Returns the parsed data field, or rejects on
  // backend error / cancellation.

  // In-flight jobId per analysis type — used to cancel a prior run when the
  // user triggers the same analysis again before the first finishes.
  const inFlight = { cutDetect: null, blackDetect: null, loudness: null, frameMd5: null, vmaf: null };

  async function invokeAnalysis(command, params, trackerKey = null) {
    const jobId = crypto.randomUUID();

    // Cancel any previous in-flight run of this analysis type.
    if (trackerKey && inFlight[trackerKey]) {
      try { await invoke('cancel_job', { jobId: inFlight[trackerKey] }); } catch {}
    }
    if (trackerKey) inFlight[trackerKey] = jobId;

    return new Promise((resolve, reject) => {
      let unlistenFn = null;
      let settled = false;

      const settle = (fn, val) => {
        if (settled) return;
        settled = true;
        if (unlistenFn) unlistenFn();
        if (trackerKey && inFlight[trackerKey] === jobId) inFlight[trackerKey] = null;
        fn(val);
      };

      listen(`analysis-result:${jobId}`, (ev) => {
        const p = ev.payload ?? {};
        if (p.cancelled) return settle(reject, 'CANCELLED');
        if (p.error)     return settle(reject, p.error);
        // Commands define their payload as either { data, ... } or a named
        // field. Prefer `data`, else look for common named fields.
        const data = p.data ?? p.cuts ?? p.intervals ?? p;
        settle(resolve, data);
      }).then((fn) => {
        if (settled) { fn(); return; }
        unlistenFn = fn;
      }).catch((err) => settle(reject, err));

      invoke(command, { jobId, ...params }).catch((err) => settle(reject, err));
    });
  }

  // ── Analysis runners ──────────────────────────────────────────────────────

  async function runLoudness() {
    if (!selectedItem) { setStatus('Select a file first', 'error'); return; }
    const target = loudnessTarget === 'custom'
      ? Number(loudnessTargetCustom)
      : Number(loudnessTarget);
    loudnessResult = null;
    setStatus('Measuring loudness…', 'info');
    try {
      loudnessResult = await invokeAnalysis('analyze_loudness', {
        inputPath: selectedItem.path,
        targetI: target,
        targetTp: -2.0,
        truePeak: loudnessTruePeak,
      }, 'loudness');
      setStatus('Loudness measured', 'success');
    } catch (err) {
      if (err === 'CANCELLED') return;
      setStatus(`Loudness failed: ${err}`, 'error');
    }
  }

  async function runAudioNorm() {
    if (!selectedItem) { setStatus('Select a file first', 'error'); return; }
    if (selectedItem.status === 'converting') return;
    const outExt = selectedItem.ext || (selectedItem.mediaType === 'video' ? 'mp4' : 'wav');
    const outPath = expectedOutputPath(selectedItem, outExt, 'normalized', outputDir, outputSeparator);
    markConverting(selectedItem);
    try {
      await invoke('run_operation', {
        jobId: selectedItem.id,
        operation: {
          type: 'audio_normalize',
          input_path: selectedItem.path,
          output_path: outPath,
          mode: audioNormMode,
          target_i: Number(audioNormTargetI),
          target_tp: Number(audioNormTargetTP),
          target_lra: Number(audioNormTargetLRA),
          linear: audioNormLinear,
        },
      });
    } catch (err) {
      markError(selectedItem, err);
      setStatus(`Audio normalize failed: ${err}`, 'error');
    }
  }

  async function runCutDetect() {
    if (!selectedItem) { setStatus('Select a file first', 'error'); return; }
    cutDetectResults = [];
    setStatus('Detecting cuts…', 'info');
    try {
      const res = await invokeAnalysis('analyze_cut_detect', {
        inputPath: selectedItem.path,
        algo: cutDetectAlgo,
        threshold: Number(cutDetectThreshold),
        minShotS: Number(cutDetectMinShotS),
      }, 'cutDetect');
      cutDetectResults = (res ?? []).map(c => ({
        time: c.time.toFixed(3),
        score: c.score.toFixed(2),
      }));
      setStatus(`${cutDetectResults.length} cuts`, 'success');
    } catch (err) {
      if (err === 'CANCELLED') return;
      setStatus(`Cut detect failed: ${err}`, 'error');
    }
  }

  async function runBlackDetect() {
    if (!selectedItem) { setStatus('Select a file first', 'error'); return; }
    blackDetectResults = [];
    setStatus('Detecting black frames…', 'info');
    try {
      const res = await invokeAnalysis('analyze_black_detect', {
        inputPath: selectedItem.path,
        minDuration: Number(blackDetectMinDur),
        pixTh: Number(blackDetectPixTh),
        picTh: Number(blackDetectPicTh),
      }, 'blackDetect');
      blackDetectResults = (res ?? []).map(b => ({
        start: b.start.toFixed(3),
        end: b.end.toFixed(3),
        duration: b.duration.toFixed(3),
      }));
      setStatus(`${blackDetectResults.length} black intervals`, 'success');
    } catch (err) {
      if (err === 'CANCELLED') return;
      setStatus(`Black detect failed: ${err}`, 'error');
    }
  }

  async function runVmaf() {
    if (!vmafReferencePath || !vmafDistortedPath) return;
    vmafResult = null;
    setStatus('Computing VMAF…', 'info');
    try {
      const r = await invokeAnalysis('analyze_vmaf', {
        referencePath: vmafReferencePath,
        distortedPath: vmafDistortedPath,
        model: vmafModel,
        subsample: Number(vmafSubsample),
      }, 'vmaf');
      vmafResult = {
        mean: r.mean.toFixed(2),
        min: r.min.toFixed(2),
        max: r.max.toFixed(2),
        harmonic_mean: r.harmonic_mean.toFixed(2),
      };
      setStatus('VMAF complete', 'success');
    } catch (err) {
      if (err === 'CANCELLED') return;
      setStatus(`VMAF failed: ${err}`, 'error');
    }
  }

  async function runFrameMd5() {
    if (!selectedItem) { setStatus('Select a file first', 'error'); return; }
    frameMd5Result = null;
    setStatus('Hashing frames…', 'info');
    try {
      const r = await invokeAnalysis('analyze_framemd5', {
        inputPath: selectedItem.path,
        stream: frameMd5Stream,
        diffPath: frameMd5DiffPath,
      }, 'frameMd5');
      frameMd5Result = {
        hashes: r.hashes,
        firstDivergence: r.first_divergence,
      };
      setStatus('FrameMD5 complete', 'success');
    } catch (err) {
      if (err === 'CANCELLED') return;
      setStatus(`FrameMD5 failed: ${err}`, 'error');
    }
  }
</script>

{#if selectedOperation === 'loudness'}
  <!-- Row 1: target preset + true-peak toggle -->
  <div class="flex flex-wrap items-center gap-2 w-full">
    <div class="flex items-center gap-1.5 rounded border border-[var(--border)] px-2 py-1">
      <label class="text-[10px] uppercase tracking-wider text-white/40 font-semibold">Target</label>
      <select bind:value={loudnessTarget}
              class="bg-transparent text-[12px] text-white outline-none font-mono tabular-nums">
        <option value="-23">-23 LUFS · Broadcast (EBU R128)</option>
        <option value="-16">-16 LUFS · Streaming</option>
        <option value="-14">-14 LUFS · Spotify / Apple Music</option>
        <option value="custom">Custom…</option>
      </select>
    </div>
    {#if loudnessTarget === 'custom'}
      <div class="flex items-center gap-1.5 rounded border border-[var(--border)] px-2 py-1">
        <label class="text-[10px] uppercase tracking-wider text-white/40 font-semibold">LUFS</label>
        <input type="number" step="0.5" bind:value={loudnessTargetCustom}
               class="w-16 bg-transparent text-[12px] text-white outline-none text-right font-mono tabular-nums"/>
      </div>
    {/if}
    <label class="flex items-center gap-1.5 rounded border border-[var(--border)] px-2 py-1 cursor-pointer"
           title="4× oversampling for accurate true-peak measurement. Slower but required for broadcast compliance.">
      <input type="checkbox" bind:checked={loudnessTruePeak} class="accent-[var(--accent)]"/>
      <span class="text-[11px] text-white/70 font-medium">True peak</span>
    </label>
  </div>
  <!-- Row 2: run + results -->
  <div class="w-full">
    <button
      onclick={runLoudness}
      disabled={!selectedItem}
      class="px-3 py-1.5 rounded text-[12px] font-semibold bg-[var(--accent)] text-white hover:opacity-90 transition-opacity disabled:opacity-40"
    >Analyze</button>
  </div>
  <div class="w-full rounded border border-[var(--border)] bg-black/20 px-3 py-2 font-mono tabular-nums text-[12px] text-white/70">
    {#if loudnessResult}
      <div class="grid grid-cols-4 gap-2">
        <div><span class="text-white/40 text-[10px] uppercase tracking-wider block">Integrated</span>{loudnessResult.I} LUFS</div>
        <div><span class="text-white/40 text-[10px] uppercase tracking-wider block">LRA</span>{loudnessResult.LRA} LU</div>
        <div><span class="text-white/40 text-[10px] uppercase tracking-wider block">True peak</span>{loudnessResult.TP} dBTP</div>
        <div><span class="text-white/40 text-[10px] uppercase tracking-wider block">Threshold</span>{loudnessResult.threshold} LUFS</div>
      </div>
    {:else}
      <span class="text-white/30">No results yet — Analyze to measure.</span>
    {/if}
  </div>

{:else if selectedOperation === 'audio-norm'}
  <!-- Row 1: mode segmented -->
  <div class="flex flex-wrap items-center gap-2 w-full">
    <div class="inline-flex items-center rounded-md overflow-hidden border border-[var(--border)]">
      <span class="px-2 text-[10px] uppercase tracking-wider text-white/40 font-semibold">Mode</span>
      <div class="w-px h-6 bg-[var(--border)]"></div>
      <button onclick={() => audioNormMode = 'ebu'}
              title="Two-pass EBU R128 loudnorm. Most accurate; preserves dynamics with linear=true."
              class="px-2.5 py-1.5 text-[11px] font-semibold transition-colors
                     {audioNormMode === 'ebu' ? 'bg-[var(--accent)] text-white' : 'text-white/60 hover:bg-white/5'}">EBU R128</button>
      <div class="w-px h-6 bg-[var(--border)]"></div>
      <button onclick={() => audioNormMode = 'peak'}
              title="Scale peak sample to a dBFS ceiling. Fast, no perceptual weighting."
              class="px-2.5 py-1.5 text-[11px] font-semibold transition-colors
                     {audioNormMode === 'peak' ? 'bg-[var(--accent)] text-white' : 'text-white/60 hover:bg-white/5'}">Peak</button>
      <div class="w-px h-6 bg-[var(--border)]"></div>
      <button onclick={() => audioNormMode = 'rg'}
              title="Write ReplayGain tags only. No sample changes. Players apply the gain on playback."
              class="px-2.5 py-1.5 text-[11px] font-semibold transition-colors
                     {audioNormMode === 'rg' ? 'bg-[var(--accent)] text-white' : 'text-white/60 hover:bg-white/5'}">ReplayGain tag</button>
    </div>
  </div>
  <!-- Row 2: targets (EBU only shows LRA) -->
  <div class="flex flex-wrap items-center gap-2 w-full">
    <div class="flex items-center gap-1.5 rounded border border-[var(--border)] px-2 py-1">
      <label class="text-[10px] uppercase tracking-wider text-white/40 font-semibold">{audioNormMode === 'peak' ? 'dBFS' : 'I'}</label>
      <input type="number" step="0.5" bind:value={audioNormTargetI}
             class="w-16 bg-transparent text-[12px] text-white outline-none text-right font-mono tabular-nums"/>
    </div>
    {#if audioNormMode === 'ebu'}
      <div class="flex items-center gap-1.5 rounded border border-[var(--border)] px-2 py-1">
        <label class="text-[10px] uppercase tracking-wider text-white/40 font-semibold">TP</label>
        <input type="number" step="0.1" bind:value={audioNormTargetTP}
               class="w-14 bg-transparent text-[12px] text-white outline-none text-right font-mono tabular-nums"/>
      </div>
      <div class="flex items-center gap-1.5 rounded border border-[var(--border)] px-2 py-1">
        <label class="text-[10px] uppercase tracking-wider text-white/40 font-semibold">LRA</label>
        <input type="number" step="0.5" bind:value={audioNormTargetLRA}
               class="w-14 bg-transparent text-[12px] text-white outline-none text-right font-mono tabular-nums"/>
      </div>
      <label class="flex items-center gap-1.5 rounded border border-[var(--border)] px-2 py-1 cursor-pointer"
             title="Two-pass with linear=true preserves dynamic range. Single-pass is dynamic compression.">
        <input type="checkbox" bind:checked={audioNormLinear} class="accent-[var(--accent)]"/>
        <span class="text-[11px] text-white/70 font-medium">Linear (preserve dynamics)</span>
      </label>
    {/if}
  </div>
  <!-- Row 3: run -->
  <div class="w-full">
    <button
      onclick={runAudioNorm}
      disabled={!selectedItem || selectedItem.status === 'converting'}
      class="px-3 py-1.5 rounded text-[12px] font-semibold bg-[var(--accent)] text-white hover:opacity-90 transition-opacity disabled:opacity-40"
    >Run Normalize</button>
  </div>

{:else if selectedOperation === 'cut-detect'}
  <!-- Row 1: algo + threshold + min shot -->
  <div class="flex flex-wrap items-center gap-2 w-full">
    <div class="inline-flex items-center rounded-md overflow-hidden border border-[var(--border)]">
      <span class="px-2 text-[10px] uppercase tracking-wider text-white/40 font-semibold">Algo</span>
      <div class="w-px h-6 bg-[var(--border)]"></div>
      <button onclick={() => cutDetectAlgo = 'scdet'}
              title="FFmpeg scdet filter. Modern scoring, range ~5–15."
              class="px-2.5 py-1.5 text-[11px] font-semibold transition-colors
                     {cutDetectAlgo === 'scdet' ? 'bg-[var(--accent)] text-white' : 'text-white/60 hover:bg-white/5'}">scdet</button>
      <div class="w-px h-6 bg-[var(--border)]"></div>
      <button onclick={() => cutDetectAlgo = 'scene'}
              title="Classic select='gt(scene,T)'. Range 0.2–0.5."
              class="px-2.5 py-1.5 text-[11px] font-semibold transition-colors
                     {cutDetectAlgo === 'scene' ? 'bg-[var(--accent)] text-white' : 'text-white/60 hover:bg-white/5'}">scene</button>
    </div>
    <div class="flex items-center gap-1.5 rounded border border-[var(--border)] px-2 py-1">
      <label class="text-[10px] uppercase tracking-wider text-white/40 font-semibold">Threshold</label>
      <input type="number" step="0.5" bind:value={cutDetectThreshold}
             class="w-16 bg-transparent text-[12px] text-white outline-none text-right font-mono tabular-nums"/>
    </div>
    <div class="flex items-center gap-1.5 rounded border border-[var(--border)] px-2 py-1">
      <label class="text-[10px] uppercase tracking-wider text-white/40 font-semibold">Min shot (s)</label>
      <input type="number" step="0.1" bind:value={cutDetectMinShotS}
             class="w-14 bg-transparent text-[12px] text-white outline-none text-right font-mono tabular-nums"/>
    </div>
  </div>
  <!-- Row 2: run -->
  <div class="w-full">
    <button
      onclick={runCutDetect}
      disabled={!selectedItem}
      class="px-3 py-1.5 rounded text-[12px] font-semibold bg-[var(--accent)] text-white hover:opacity-90 transition-opacity disabled:opacity-40"
    >Detect Cuts</button>
  </div>
  <!-- Row 3: results -->
  <div class="w-full rounded border border-[var(--border)] bg-black/20 px-3 py-2 font-mono tabular-nums text-[12px] max-h-40 overflow-y-auto">
    {#if cutDetectResults.length}
      {#each cutDetectResults as c, i}
        <button class="w-full text-left py-0.5 px-1 hover:bg-white/5 text-white/70 flex justify-between">
          <span>#{i + 1}</span>
          <span>{c.time}s</span>
          <span class="text-white/40">{c.score}</span>
        </button>
      {/each}
    {:else}
      <span class="text-white/30">No cuts detected yet — run to populate.</span>
    {/if}
  </div>

{:else if selectedOperation === 'black-detect'}
  <!-- Row 1: thresholds -->
  <div class="flex flex-wrap items-center gap-2 w-full">
    <div class="flex items-center gap-1.5 rounded border border-[var(--border)] px-2 py-1">
      <label class="text-[10px] uppercase tracking-wider text-white/40 font-semibold">Min dur (s)</label>
      <input type="number" step="0.05" bind:value={blackDetectMinDur}
             class="w-16 bg-transparent text-[12px] text-white outline-none text-right font-mono tabular-nums"/>
    </div>
    <div class="flex items-center gap-1.5 rounded border border-[var(--border)] px-2 py-1"
         title="pix_th: pixel luma threshold below which a pixel counts as black.">
      <label class="text-[10px] uppercase tracking-wider text-white/40 font-semibold">Pix th</label>
      <input type="number" step="0.01" min="0" max="1" bind:value={blackDetectPixTh}
             class="w-16 bg-transparent text-[12px] text-white outline-none text-right font-mono tabular-nums"/>
    </div>
    <div class="flex items-center gap-1.5 rounded border border-[var(--border)] px-2 py-1"
         title="pic_th: fraction of pixels below pix_th for a frame to count as black.">
      <label class="text-[10px] uppercase tracking-wider text-white/40 font-semibold">Pic th</label>
      <input type="number" step="0.01" min="0" max="1" bind:value={blackDetectPicTh}
             class="w-16 bg-transparent text-[12px] text-white outline-none text-right font-mono tabular-nums"/>
    </div>
  </div>
  <!-- Row 2: run -->
  <div class="w-full">
    <button
      onclick={runBlackDetect}
      disabled={!selectedItem}
      class="px-3 py-1.5 rounded text-[12px] font-semibold bg-[var(--accent)] text-white hover:opacity-90 transition-opacity disabled:opacity-40"
    >Detect Black</button>
  </div>
  <!-- Row 3: results -->
  <div class="w-full rounded border border-[var(--border)] bg-black/20 px-3 py-2 font-mono tabular-nums text-[12px] max-h-40 overflow-y-auto">
    {#if blackDetectResults.length}
      {#each blackDetectResults as b, i}
        <button class="w-full text-left py-0.5 px-1 hover:bg-white/5 text-white/70 grid grid-cols-4 gap-2">
          <span>#{i + 1}</span>
          <span>{b.start}s</span>
          <span>{b.end}s</span>
          <span class="text-white/40">{b.duration}s</span>
        </button>
      {/each}
    {:else}
      <span class="text-white/30">No black intervals detected yet.</span>
    {/if}
  </div>

{:else if selectedOperation === 'vmaf'}
  <!-- Row 1: file pickers -->
  <div class="flex flex-wrap items-center gap-2 w-full">
    <button onclick={() => vmafReferencePath
              ? (vmafReferencePath = null)
              : pickAuxFile((p) => vmafReferencePath = p, 'video/*')}
            class="px-3 py-1.5 rounded text-[12px] font-medium border border-[var(--border)] text-[var(--text-secondary)] hover:text-[var(--text-primary)] hover:border-[var(--accent)] transition-colors">
      {vmafReferencePath ? 'Clear reference' : 'Pick reference…'}
    </button>
    <button onclick={() => vmafDistortedPath
              ? (vmafDistortedPath = null)
              : pickAuxFile((p) => vmafDistortedPath = p, 'video/*')}
            class="px-3 py-1.5 rounded text-[12px] font-medium border border-[var(--border)] text-[var(--text-secondary)] hover:text-[var(--text-primary)] hover:border-[var(--accent)] transition-colors">
      {vmafDistortedPath ? 'Clear distorted' : 'Pick distorted…'}
    </button>
  </div>
  <!-- Row 2: model + subsample -->
  <div class="flex flex-wrap items-center gap-2 w-full">
    <div class="flex items-center gap-1.5 rounded border border-[var(--border)] px-2 py-1">
      <label class="text-[10px] uppercase tracking-wider text-white/40 font-semibold">Model</label>
      <select bind:value={vmafModel}
              class="bg-transparent text-[12px] text-white outline-none">
        <option value="hd">HD · vmaf_v0.6.1</option>
        <option value="4k">4K · vmaf_4k_v0.6.1</option>
        <option value="phone">Phone</option>
      </select>
    </div>
    <div class="flex items-center gap-1.5 rounded border border-[var(--border)] px-2 py-1"
         title="Score every Nth frame. 1 = every frame (slowest, most accurate). 5 = 5× faster.">
      <label class="text-[10px] uppercase tracking-wider text-white/40 font-semibold">Subsample</label>
      <input type="number" step="1" min="1" bind:value={vmafSubsample}
             class="w-12 bg-transparent text-[12px] text-white outline-none text-right font-mono tabular-nums"/>
    </div>
  </div>
  <!-- Row 3: run + result -->
  <div class="w-full">
    <button
      onclick={runVmaf}
      disabled={!vmafReferencePath || !vmafDistortedPath}
      class="px-3 py-1.5 rounded text-[12px] font-semibold bg-[var(--accent)] text-white hover:opacity-90 transition-opacity disabled:opacity-40"
    >Run VMAF</button>
  </div>
  <div class="w-full rounded border border-[var(--border)] bg-black/20 px-3 py-2 font-mono tabular-nums text-[12px] text-white/70">
    {#if vmafResult}
      <div class="grid grid-cols-4 gap-2">
        <div><span class="text-white/40 text-[10px] uppercase tracking-wider block">Mean</span>{vmafResult.mean}</div>
        <div><span class="text-white/40 text-[10px] uppercase tracking-wider block">Harmonic</span>{vmafResult.harmonic_mean}</div>
        <div><span class="text-white/40 text-[10px] uppercase tracking-wider block">Min</span>{vmafResult.min}</div>
        <div><span class="text-white/40 text-[10px] uppercase tracking-wider block">Max</span>{vmafResult.max}</div>
      </div>
    {:else}
      <span class="text-white/30">Drop a reference and distorted file, then Run.</span>
    {/if}
  </div>

{:else if selectedOperation === 'framemd5'}
  <!-- Row 1: stream picker + optional diff file -->
  <div class="flex flex-wrap items-center gap-2 w-full">
    <div class="inline-flex items-center rounded-md overflow-hidden border border-[var(--border)]">
      <span class="px-2 text-[10px] uppercase tracking-wider text-white/40 font-semibold">Streams</span>
      <div class="w-px h-6 bg-[var(--border)]"></div>
      <button onclick={() => frameMd5Stream = 'video'}
              class="px-2.5 py-1.5 text-[11px] font-semibold transition-colors
                     {frameMd5Stream === 'video' ? 'bg-[var(--accent)] text-white' : 'text-white/60 hover:bg-white/5'}">Video</button>
      <div class="w-px h-6 bg-[var(--border)]"></div>
      <button onclick={() => frameMd5Stream = 'audio'}
              class="px-2.5 py-1.5 text-[11px] font-semibold transition-colors
                     {frameMd5Stream === 'audio' ? 'bg-[var(--accent)] text-white' : 'text-white/60 hover:bg-white/5'}">Audio</button>
      <div class="w-px h-6 bg-[var(--border)]"></div>
      <button onclick={() => frameMd5Stream = 'both'}
              class="px-2.5 py-1.5 text-[11px] font-semibold transition-colors
                     {frameMd5Stream === 'both' ? 'bg-[var(--accent)] text-white' : 'text-white/60 hover:bg-white/5'}">Both</button>
    </div>
    <button onclick={() => frameMd5DiffPath
              ? (frameMd5DiffPath = null)
              : pickAuxFile((p) => frameMd5DiffPath = p, 'audio/*,video/*')}
            title="Optional second file. When set, first differing frame is highlighted."
            class="px-3 py-1.5 rounded text-[12px] font-medium border border-[var(--border)] text-[var(--text-secondary)] hover:text-[var(--text-primary)] hover:border-[var(--accent)] transition-colors">
      {frameMd5DiffPath ? 'Clear diff file' : 'Add diff file…'}
    </button>
  </div>
  <!-- Row 2: run -->
  <div class="w-full">
    <button
      onclick={runFrameMd5}
      disabled={!selectedItem}
      class="px-3 py-1.5 rounded text-[12px] font-semibold bg-[var(--accent)] text-white hover:opacity-90 transition-opacity disabled:opacity-40"
    >{frameMd5DiffPath ? 'Run Diff' : 'Hash Frames'}</button>
  </div>
  <!-- Row 3: results -->
  <div class="w-full rounded border border-[var(--border)] bg-black/20 px-3 py-2 font-mono tabular-nums text-[11px] text-white/70 max-h-40 overflow-y-auto">
    {#if frameMd5Result}
      {#if frameMd5Result.firstDivergence != null}
        <div class="text-[var(--accent)] mb-1">First divergence at frame #{frameMd5Result.firstDivergence}</div>
      {/if}
      {#each frameMd5Result.hashes ?? [] as h}
        <div class="flex gap-2"><span class="text-white/30 w-10 shrink-0">{h.idx}</span><span>{h.hash}</span></div>
      {/each}
    {:else}
      <span class="text-white/30">No hashes yet — run to populate.</span>
    {/if}
  </div>
{/if}
