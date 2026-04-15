<script>
  import { convertFileSrc, invoke } from '@tauri-apps/api/core';

  let { item, duration = null, options = $bindable(null) } = $props();

  // ── Audio element ─────────────────────────────────────────────────────────
  let audioEl     = $state(null);
  let isPlaying   = $state(false);
  let currentTime = $state(0);
  let _prevAudio  = null;

  $effect(() => {
    const it = item;
    if (_prevAudio) { _prevAudio.pause(); _prevAudio.src = ''; _prevAudio = null; }
    _teardownGraph();
    isPlaying = false; currentTime = 0; audioEl = null;
    if (!it) return;
    try {
      const a = new Audio(convertFileSrc(it.path));
      a.addEventListener('timeupdate', () => { currentTime = a.currentTime; });
      a.addEventListener('ended',      () => { isPlaying = false; currentTime = 0; });
      audioEl = a; _prevAudio = a;
    } catch { /* non-fatal */ }
    return () => {
      if (_prevAudio) { _prevAudio.pause(); _prevAudio.src = ''; _prevAudio = null; }
      _teardownGraph();
    };
  });

  // ── Web Audio graph ───────────────────────────────────────────────────────
  let _audioCtx  = null;
  let _analyserL = null; // left channel → Lissajous X + spectrum L
  let _analyserR = null; // right channel → Lissajous Y + spectrum R

  function _teardownGraph() {
    if (_rafId) { cancelAnimationFrame(_rafId); _rafId = null; }
    if (_audioCtx) { _audioCtx.close().catch(() => {}); _audioCtx = null; }
    _analyserL = _analyserR = null;
  }

  function _buildGraph() {
    if (!audioEl || _audioCtx) return;
    try {
      const ctx = new AudioContext();
      const src = ctx.createMediaElementSource(audioEl);

      const splitter = ctx.createChannelSplitter(2);
      const aL = ctx.createAnalyser();
      const aR = ctx.createAnalyser();
      aL.fftSize = aR.fftSize = 2048;
      aL.smoothingTimeConstant = aR.smoothingTimeConstant = 0.75;

      src.connect(ctx.destination);
      src.connect(splitter);
      splitter.connect(aL, 0);
      splitter.connect(aR, 1);

      _audioCtx = ctx; _analyserL = aL; _analyserR = aR;
    } catch (e) { console.error('AudioContext setup failed:', e); }
  }

  // ── Playback controls ─────────────────────────────────────────────────────
  function togglePlay() {
    if (!audioEl) return;
    if (isPlaying) {
      audioEl.pause(); isPlaying = false;
    } else {
      _buildGraph();
      _audioCtx?.resume();
      if (options?.trim_start != null) audioEl.currentTime = options.trim_start;
      audioEl.play().then(() => isPlaying = true).catch(() => {});
    }
  }

  function seekTo(secs) {
    currentTime = Math.max(0, Math.min(duration ?? 0, secs));
    if (audioEl) audioEl.currentTime = currentTime;
  }

  // ── Canvas refs ───────────────────────────────────────────────────────────
  let lissajousCanvas    = $state(null);
  let oscilloscopeCanvas = $state(null);
  let spectrumCanvas     = $state(null);
  let vuCanvas           = $state(null);

  // VU meter peak-hold state (plain vars — not reactive)
  let _peakL = -60, _peakR = -60;
  let _peakHoldL = 0, _peakHoldR = 0;
  let _rafId = null;

  // Lissajous — trail-faded XY scatter, L=X, R=Y
  function _drawLissajous() {
    const cv = lissajousCanvas;
    if (!cv || !_analyserL || !_analyserR) return;
    const ctx = cv.getContext('2d');
    const w = cv.width, h = cv.height, cx = w / 2, cy = h / 2;
    const n   = _analyserL.fftSize;
    const lBuf = new Float32Array(n);
    const rBuf = new Float32Array(n);
    _analyserL.getFloatTimeDomainData(lBuf);
    _analyserR.getFloatTimeDomainData(rBuf);

    // Fade previous frame (creates persistence/trail)
    ctx.fillStyle = 'rgba(0,0,0,0.28)';
    ctx.fillRect(0, 0, w, h);

    // Faint crosshairs
    ctx.strokeStyle = 'rgba(255,255,255,0.07)';
    ctx.lineWidth = 1;
    ctx.beginPath();
    ctx.moveTo(cx, 0); ctx.lineTo(cx, h);
    ctx.moveTo(0, cy); ctx.lineTo(w, cy);
    ctx.stroke();

    // Plot each sample as a dot, colour by instantaneous magnitude
    for (let i = 0; i < n; i++) {
      const x = cx + lBuf[i] * cx * 0.88;
      const y = cy - rBuf[i] * cy * 0.88;
      const mag    = Math.sqrt(lBuf[i] * lBuf[i] + rBuf[i] * rBuf[i]);
      const bright = Math.min(1, mag * 2.5);
      const g = Math.round(140 + bright * 115);
      const b = Math.round(60  + bright * 130);
      ctx.fillStyle = `rgba(20,${g},${b},0.65)`;
      ctx.fillRect(x - 0.5, y - 0.5, 1.5, 1.5);
    }

    // ── Axis labels & ticks (drawn on top of dots) ───────────────────────
    // Canvas is 216×216 displayed at 108×108 — use 2× font sizes throughout
    ctx.font = '18px monospace';
    ctx.fillStyle = 'rgba(255,255,255,0.55)';

    ctx.textAlign = 'right'; ctx.textBaseline = 'middle';
    ctx.fillText('L+', w - 4, cy);
    ctx.textAlign = 'left';
    ctx.fillText('L−', 4, cy);
    ctx.textAlign = 'center'; ctx.textBaseline = 'top';
    ctx.fillText('R+', cx, 4);
    ctx.textBaseline = 'bottom';
    ctx.fillText('R−', cx, h - 2);

    // ±0.5 tick marks on both axes
    ctx.strokeStyle = 'rgba(255,255,255,0.4)';
    ctx.lineWidth = 1;
    const half = 0.5 * 0.88; // 0.5 amplitude × scale factor
    [half, -half].forEach(v => {
      const xT = cx + v * cx;
      ctx.beginPath(); ctx.moveTo(xT, cy - 7); ctx.lineTo(xT, cy + 7); ctx.stroke();
      const yT = cy - v * cy; // positive R → upward (smaller y)
      ctx.beginPath(); ctx.moveTo(cx - 7, yT); ctx.lineTo(cx + 7, yT); ctx.stroke();
    });
  }

  // Oscilloscope — L + R time-domain waveforms superimposed on a grid
  function _drawOscilloscope() {
    const cv = oscilloscopeCanvas;
    if (!cv || !_analyserL || !_analyserR) return;
    const ctx = cv.getContext('2d');
    const w = cv.width, h = cv.height;
    const n = _analyserL.fftSize;
    const lBuf = new Float32Array(n);
    const rBuf = new Float32Array(n);
    _analyserL.getFloatTimeDomainData(lBuf);
    _analyserR.getFloatTimeDomainData(rBuf);

    // Background
    ctx.fillStyle = '#080808';
    ctx.fillRect(0, 0, w, h);

    // Grid — 4 rows × 8 cols
    ctx.lineWidth = 1;
    for (let i = 0; i <= 4; i++) {
      const y = (i / 4) * h;
      ctx.strokeStyle = i === 2 ? 'rgba(255,255,255,0.13)' : 'rgba(255,255,255,0.06)';
      ctx.beginPath(); ctx.moveTo(0, y); ctx.lineTo(w, y); ctx.stroke();
    }
    for (let i = 0; i <= 8; i++) {
      const x = (i / 8) * w;
      ctx.strokeStyle = 'rgba(255,255,255,0.06)';
      ctx.beginPath(); ctx.moveTo(x, 0); ctx.lineTo(x, h); ctx.stroke();
    }

    // Draw a channel as a continuous line
    function drawChannel(buf, color) {
      ctx.strokeStyle = color;
      ctx.lineWidth = 1.5;
      ctx.beginPath();
      for (let i = 0; i < n; i++) {
        const x = (i / (n - 1)) * w;
        const y = ((1 - buf[i]) / 2) * h;
        if (i === 0) ctx.moveTo(x, y); else ctx.lineTo(x, y);
      }
      ctx.stroke();
    }

    drawChannel(lBuf, 'rgba(0, 220, 155, 0.9)');  // teal-green — L
    drawChannel(rBuf, 'rgba(60, 150, 255, 0.75)'); // blue — R
  }

  // Spectrum — L + R frequency lines superimposed, same style as oscilloscope
  function _drawSpectrum() {
    const cv = spectrumCanvas;
    if (!cv || !_analyserL || !_analyserR) return;
    const ctx = cv.getContext('2d');
    const w = cv.width, h = cv.height;
    const n = _analyserL.frequencyBinCount;
    const lData = new Uint8Array(n);
    const rData = new Uint8Array(n);
    _analyserL.getByteFrequencyData(lData);
    _analyserR.getByteFrequencyData(rData);

    // Background
    ctx.fillStyle = '#080808';
    ctx.fillRect(0, 0, w, h);

    // Grid — 4 rows × 8 cols, centre line slightly brighter
    ctx.lineWidth = 1;
    for (let i = 0; i <= 4; i++) {
      const y = (i / 4) * h;
      ctx.strokeStyle = i === 2 ? 'rgba(255,255,255,0.13)' : 'rgba(255,255,255,0.06)';
      ctx.beginPath(); ctx.moveTo(0, y); ctx.lineTo(w, y); ctx.stroke();
    }
    for (let i = 0; i <= 8; i++) {
      const x = (i / 8) * w;
      ctx.strokeStyle = 'rgba(255,255,255,0.06)';
      ctx.beginPath(); ctx.moveTo(x, 0); ctx.lineTo(x, h); ctx.stroke();
    }

    // Log-scale X mapping: spreads bass out, compresses highs naturally
    const logX = (i) => i <= 0 ? 0 : Math.log(1 + i) / Math.log(1 + n - 1) * w;

    // Draw one channel as a filled area + line on top
    function drawChannel(data, lineColor, fillColor) {
      // Filled area under the curve
      ctx.beginPath();
      ctx.moveTo(0, h);
      for (let i = 0; i < n; i++) {
        const x = logX(i);
        const y = h - (data[i] / 255) * h;
        ctx.lineTo(x, y);
      }
      ctx.lineTo(w, h);
      ctx.closePath();
      ctx.fillStyle = fillColor;
      ctx.fill();

      // Line on top
      ctx.beginPath();
      for (let i = 0; i < n; i++) {
        const x = logX(i);
        const y = h - (data[i] / 255) * h;
        if (i === 0) ctx.moveTo(x, y); else ctx.lineTo(x, y);
      }
      ctx.strokeStyle = lineColor;
      ctx.lineWidth = 1.5;
      ctx.stroke();
    }

    drawChannel(lData, 'rgba(0, 220, 155, 0.9)',  'rgba(0, 220, 155, 0.12)');
    drawChannel(rData, 'rgba(60, 150, 255, 0.75)', 'rgba(60, 150, 255, 0.10)');
  }

  // VU meter — RMS level bars with peak hold, gradient green/yellow/red
  function _drawVU() {
    const cv = vuCanvas;
    if (!cv || !_analyserL || !_analyserR) return;
    const ctx = cv.getContext('2d');
    const w = cv.width, h = cv.height;

    ctx.fillStyle = '#080808';
    ctx.fillRect(0, 0, w, h);

    const bufL = new Float32Array(_analyserL.fftSize);
    const bufR = new Float32Array(_analyserR.fftSize);
    _analyserL.getFloatTimeDomainData(bufL);
    _analyserR.getFloatTimeDomainData(bufR);

    const rmsL = Math.sqrt(bufL.reduce((s, v) => s + v * v, 0) / bufL.length);
    const rmsR = Math.sqrt(bufR.reduce((s, v) => s + v * v, 0) / bufR.length);
    const dbL = rmsL > 0 ? Math.max(-60, 20 * Math.log10(rmsL)) : -60;
    const dbR = rmsR > 0 ? Math.max(-60, 20 * Math.log10(rmsR)) : -60;

    if (dbL >= _peakL) { _peakL = dbL; _peakHoldL = 60; }
    else if (_peakHoldL > 0) _peakHoldL--;
    else _peakL = Math.max(-60, _peakL - 0.3);
    if (dbR >= _peakR) { _peakR = dbR; _peakHoldR = 60; }
    else if (_peakHoldR > 0) _peakHoldR--;
    else _peakR = Math.max(-60, _peakR - 0.3);

    const DB_MIN = -60, DB_MAX = 0;
    const toY = (db) => (1 - (db - DB_MIN) / (DB_MAX - DB_MIN)) * h;

    // Red (loud) → yellow → green (quiet) gradient
    const grad = ctx.createLinearGradient(0, 0, 0, h);
    grad.addColorStop(0,                    '#ef4444');
    grad.addColorStop(toY(-3)  / h,         '#ef4444');
    grad.addColorStop(toY(-3)  / h + 0.001, '#eab308');
    grad.addColorStop(toY(-9)  / h,         '#eab308');
    grad.addColorStop(toY(-9)  / h + 0.001, '#22c55e');
    grad.addColorStop(1,                    '#14532d');

    const L_X = 4, R_X = 54, BAR_W = 38;

    function drawBar(db, peak, x) {
      ctx.fillStyle = '#111';
      ctx.fillRect(x, 0, BAR_W, h);
      const lY = toY(db);
      if (lY < h) { ctx.fillStyle = grad; ctx.fillRect(x, lY, BAR_W, h - lY); }
      if (peak > DB_MIN) {
        const py = Math.max(0, toY(peak) - 1);
        ctx.fillStyle = peak > -3 ? '#fca5a5' : peak > -9 ? '#fde047' : '#86efac';
        ctx.fillRect(x, py, BAR_W, 2);
      }
    }

    drawBar(dbL, _peakL, L_X);
    drawBar(dbR, _peakR, R_X);

    // L / R labels (18px canvas = 9px displayed at 2× scale)
    ctx.font = '18px monospace';
    ctx.fillStyle = 'rgba(255,255,255,0.5)';
    ctx.textAlign = 'center';
    ctx.fillText('L', L_X + BAR_W / 2, 14);
    ctx.fillText('R', R_X + BAR_W / 2, 14);

    // Tick lines across both bars at key dB positions
    ctx.fillStyle = 'rgba(255,255,255,0.18)';
    [-6, -12, -24, -48].forEach(db => {
      const y = Math.round(toY(db));
      ctx.fillRect(L_X, y, BAR_W, 1);
      ctx.fillRect(R_X, y, BAR_W, 1);
    });
  }

  function _renderLoop() {
    _drawLissajous();
    _drawOscilloscope();
    _drawSpectrum();
    _drawVU();
    _rafId = requestAnimationFrame(_renderLoop);
  }

  $effect(() => {
    if (isPlaying) {
      if (!_rafId) _rafId = requestAnimationFrame(_renderLoop);
    } else {
      if (_rafId) { cancelAnimationFrame(_rafId); _rafId = null; }
    }
    return () => { if (_rafId) { cancelAnimationFrame(_rafId); _rafId = null; } };
  });

  // ── Static waveform / spectrogram (loaded via ffmpeg) ─────────────────────
  /** @type {{ amplitudes: number[], hues: number[] } | null} */
  let waveformData    = $state(null);
  let spectrogramData = $state(null);
  let mediaLoading    = $state(false);
  let _capturedId     = null;

  $effect(() => {
    const it = item;
    waveformData = null; spectrogramData = null;
    if (!it) return;
    const id = it.id; _capturedId = id; mediaLoading = true;
    invoke('get_waveform', { path: it.path })
      .then(d   => { if (_capturedId === id) { waveformData = /** @type {any} */ (d); mediaLoading = false; } })
      .catch(e  => { console.error('get_waveform failed:', e); if (_capturedId === id) mediaLoading = false; });
    invoke('get_spectrogram', { path: it.path })
      .then(b64 => { if (_capturedId === id) spectrogramData = b64; })
      .catch(() => {});
  });

  // ── Derived fractions ─────────────────────────────────────────────────────
  let startFrac = $derived(
    duration && options?.trim_start != null
      ? Math.max(0, Math.min(1, options.trim_start / duration)) : 0);
  let endFrac = $derived(
    duration && options?.trim_end != null
      ? Math.max(0, Math.min(1, options.trim_end / duration)) : 1);
  let playFrac = $derived(duration ? Math.max(0, Math.min(1, currentTime / duration)) : 0);

  // ── Drag ──────────────────────────────────────────────────────────────────
  let trackEl  = $state(null);
  let dragging = $state(null); // 'start' | 'end' | 'playhead'

  function getFrac(e) {
    if (!trackEl) return 0;
    const r = trackEl.getBoundingClientRect();
    return Math.max(0, Math.min(1, (e.clientX - r.left) / r.width));
  }
  function fracToSecs(f) { return f * (duration ?? 0); }
  function onTrackDown(e) {
    if (!duration) return;
    dragging = 'playhead';
    seekTo(fracToSecs(getFrac(e)));
  }
  function onWindowMouseMove(e) {
    if (!dragging || !duration) return;
    const f = getFrac(e);
    if      (dragging === 'start') options.trim_start = fracToSecs(Math.min(f, endFrac - 1 / duration));
    else if (dragging === 'end')   options.trim_end   = fracToSecs(Math.max(f, startFrac + 1 / duration));
    else                           seekTo(fracToSecs(f));
  }
  function onWindowMouseUp() { dragging = null; }

  // ── Helpers ───────────────────────────────────────────────────────────────
  function fmt(secs) {
    if (secs == null || secs < 0) return '—';
    const m = Math.floor(secs / 60);
    const s = (secs % 60).toFixed(1);
    return `${m}:${s.padStart(4, '0')}`;
  }

  const showTrim   = $derived(options != null && duration != null);
  let vizExpanded  = $state(false);
</script>

<svelte:window onmousemove={onWindowMouseMove} onmouseup={onWindowMouseUp} />

<div class="shrink-0 border-t border-[var(--border)] flex flex-col select-none"
     style="background:#0a0a0a">

  <!-- ── Advanced Audio — collapsible header ──────────────────────────────── -->
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <button class="w-full flex items-center justify-between px-4 shrink-0 select-none"
          style="height:30px; border-bottom:1px solid rgba(255,255,255,0.07); cursor:pointer"
          onclick={() => vizExpanded = !vizExpanded}>
    <span style="font:10px/1 monospace; color:rgba(255,255,255,0.4); letter-spacing:0.1em; text-transform:uppercase">
      Advanced Audio
    </span>
    <svg width="20" height="14" viewBox="0 0 20 14" fill="none"
         stroke="rgba(255,255,255,0.4)" stroke-width="2"
         stroke-linecap="round" stroke-linejoin="round">
      {#if vizExpanded}
        <path d="M1 13 L10 7 L19 13"/>
        <path d="M1 7  L10 1 L19 7"/>
      {:else}
        <path d="M1 1 L10 7 L19 1"/>
        <path d="M1 7 L10 13 L19 7"/>
      {/if}
    </svg>
  </button>

  <!-- ── Advanced Audio — expandable content ──────────────────────────────── -->
  {#if vizExpanded}
    {#if spectrogramData}
      <div class="shrink-0 px-3 pt-2" style="height:224px">
        <div class="relative w-full h-full">
          <img src="data:image/png;base64,{spectrogramData}" alt="Spectrogram"
               class="w-full h-full object-fill rounded" />
          <div class="absolute inset-0 pointer-events-none rounded overflow-hidden"
               style="font:9px monospace; color:rgba(255,255,255,0.55)">
            {#each [[10,'20k'],[11,'10k'],[21,'5k'],[44,'1k'],[54,'500'],[77,'100']] as [pct, lbl]}
              <span class="absolute right-1 leading-none" style="top:{pct}%; transform:translateY(-50%)">{lbl}</span>
              <span class="absolute left-0 leading-none pointer-events-none"
                    style="top:{pct}%; border-top:1px solid rgba(255,255,255,0.18); width:6px"></span>
            {/each}
          </div>
        </div>
      </div>
    {/if}

    <div class="shrink-0 flex gap-2 px-3 pt-2 pb-2" style="height:120px">
      <canvas bind:this={lissajousCanvas} width="216" height="216"
              style="width:108px; height:108px; border-radius:4px; background:#000; flex-shrink:0; display:block"
      ></canvas>
      <div class="flex-1 relative" style="height:108px; min-width:0">
        <canvas bind:this={oscilloscopeCanvas} width="1024" height="216"
                style="width:100%; height:108px; border-radius:4px; display:block; background:#080808"
        ></canvas>
        <div class="absolute inset-0 pointer-events-none flex flex-col justify-between py-px pl-1 rounded overflow-hidden"
             style="font:9px monospace; color:rgba(255,255,255,0.5)">
          <span>+1</span><span>+.5</span><span style="color:rgba(255,255,255,0.75)">0</span><span>−.5</span><span>−1</span>
        </div>
        <div class="absolute top-1 right-1 pointer-events-none flex flex-col gap-px" style="font:9px monospace">
          <span style="color:rgba(0,220,155,0.9)">L</span><span style="color:rgba(60,150,255,0.85)">R</span>
        </div>
      </div>
      <div class="flex-1 relative" style="height:108px; min-width:0">
        <canvas bind:this={spectrumCanvas} width="1024" height="216"
                style="width:100%; height:108px; border-radius:4px; display:block; background:#000"
        ></canvas>
        <div class="absolute inset-0 pointer-events-none rounded overflow-hidden">
          {#each [[25,'100'],[46,'500'],[56,'1k'],[79,'5k'],[89,'10k']] as [pct, lbl]}
            <div class="absolute top-0 bottom-0" style="left:{pct}%; border-left:1px solid rgba(255,255,255,0.18)"></div>
            <span class="absolute bottom-0.5 leading-none"
                  style="left:{pct}%; transform:translateX(-50%); font:9px monospace; color:rgba(255,255,255,0.55)">{lbl}</span>
          {/each}
          <div class="absolute top-1 right-1 flex flex-col gap-px" style="font:9px monospace">
            <span style="color:rgba(0,220,155,0.9)">L</span><span style="color:rgba(60,150,255,0.85)">R</span>
          </div>
        </div>
      </div>
    </div>
  {/if}

  <!-- ── Main timeline: [waveform + controls] | [VU meter] ──────────────── -->
  <div class="shrink-0 flex" style="height:176px">

    <!-- Left column: waveform track + controls -->
    <div class="flex-1 min-w-0 flex flex-col">

      <!-- Track (waveform + trim + playhead) -->
      <!-- svelte-ignore a11y_no_static_element_interactions -->
      <div bind:this={trackEl}
           class="flex-1 min-h-0 relative mx-3 mt-1.5 mb-1 rounded cursor-crosshair"
           onmousedown={onTrackDown}>

    <!-- Background / waveform -->
    <div class="absolute inset-0 rounded overflow-hidden" style="background:#161616">
      {#if waveformData && waveformData.amplitudes.length > 0}
        <svg class="w-full h-full" preserveAspectRatio="none"
             viewBox="0 0 {waveformData.amplitudes.length} 100" xmlns="http://www.w3.org/2000/svg">
          {#each waveformData.amplitudes as amp, i}
            {@const h = Math.max(1, amp * 96)}
            {@const y = (100 - h) / 2}
            <rect x={i} y={y} width={0.85} height={h}
                  fill={`hsl(${waveformData.hues[i]},100%,55%)`} opacity="0.85" />
          {/each}
        </svg>
      {:else if mediaLoading}
        <div class="absolute inset-0 flex items-center justify-center">
          <span style="font-size:10px; color:#333">Loading…</span>
        </div>
      {/if}
    </div>

    {#if showTrim}
      <!-- Pre-trim dim -->
      <div class="absolute inset-y-0 left-0 rounded-l pointer-events-none"
           style="width:{startFrac * 100}%; background:rgba(0,0,0,0.55)"></div>
      <!-- Active region -->
      <div class="absolute inset-y-0 pointer-events-none"
           style="left:{startFrac * 100}%; width:{(endFrac - startFrac) * 100}%;
                  border-top:1px solid rgba(255,255,255,0.08);
                  border-bottom:1px solid rgba(255,255,255,0.08);
                  background:rgba(255,255,255,0.03)"></div>
      <!-- Post-trim dim -->
      <div class="absolute inset-y-0 right-0 rounded-r pointer-events-none"
           style="left:{endFrac * 100}%; background:rgba(0,0,0,0.55)"></div>
      <!-- Left handle -->
      <!-- svelte-ignore a11y_no_static_element_interactions -->
      <div class="absolute inset-y-0 z-20 flex items-center justify-center cursor-ew-resize"
           style="left:calc({startFrac * 100}% - 6px); width:12px"
           onmousedown={(e) => { e.stopPropagation(); dragging = 'start'; }}>
        <div class="w-[3px] h-full rounded-full transition-colors"
             style="background:{dragging === 'start' ? '#ddd' : '#666'}"></div>
      </div>
      <!-- Right handle -->
      <!-- svelte-ignore a11y_no_static_element_interactions -->
      <div class="absolute inset-y-0 z-20 flex items-center justify-center cursor-ew-resize"
           style="left:calc({endFrac * 100}% - 6px); width:12px"
           onmousedown={(e) => { e.stopPropagation(); dragging = 'end'; }}>
        <div class="w-[3px] h-full rounded-full transition-colors"
             style="background:{dragging === 'end' ? '#ddd' : '#666'}"></div>
      </div>
    {/if}

    <!-- Playhead -->
    {#if duration}
      <!-- svelte-ignore a11y_no_static_element_interactions -->
      <div class="absolute inset-y-0 z-30 cursor-ew-resize"
           style="left:{playFrac * 100}%; transform:translateX(-50%)"
           onmousedown={(e) => { e.stopPropagation(); dragging = 'playhead'; }}>
        <div class="absolute left-1/2 -translate-x-1/2 w-2.5 h-2.5 rounded-full"
             style="top:-5px; background:rgba(255,255,255,0.9); box-shadow:0 0 5px rgba(255,255,255,0.5)"></div>
        <div class="absolute top-0 bottom-0 left-1/2 -translate-x-px w-px"
             style="background:rgba(255,255,255,0.8)"></div>
      </div>
    {/if}
      </div><!-- /track -->

      <!-- Controls row -->
      <div class="relative flex items-center px-3 shrink-0" style="height:64px">

    <!-- Trim range — left -->
    {#if showTrim}
      <div class="absolute left-3 flex items-center gap-1 font-mono tabular-nums" style="font-size:12px; color:white">
        <span style="opacity:{startFrac > 0 ? 1 : 0.4}">{fmt(options.trim_start ?? 0)}</span>
        <span style="opacity:0.3">–</span>
        <span style="opacity:{endFrac < 1 ? 1 : 0.4}">{fmt(options.trim_end ?? duration)}</span>
      </div>
    {/if}

    <!-- Playback buttons — centred -->
    <div class="absolute left-1/2 -translate-x-1/2 flex items-center gap-4">
      <button onclick={() => seekTo(options?.trim_start ?? 0)}
              class="w-12 h-12 flex items-center justify-center rounded opacity-40 hover:opacity-100 transition-opacity"
              style="color:white" title="Go to start">
        <svg width="22" height="22" viewBox="0 0 24 24" fill="currentColor">
          <path d="M6 6h2v12H6zm3.5 6 8.5 6V6z"/>
        </svg>
      </button>

      <button onclick={togglePlay}
              class="w-14 h-14 flex items-center justify-center rounded-full"
              style="background:var(--accent)" title={isPlaying ? 'Pause' : 'Play'}>
        {#if isPlaying}
          <svg width="20" height="20" viewBox="0 0 24 24" fill="white">
            <rect x="6" y="4" width="4" height="16"/><rect x="14" y="4" width="4" height="16"/>
          </svg>
        {:else}
          <svg width="20" height="20" viewBox="0 0 24 24" fill="white">
            <path d="M8 5v14l11-7z"/>
          </svg>
        {/if}
      </button>

      <button onclick={() => seekTo(options?.trim_end ?? duration ?? 0)}
              class="w-12 h-12 flex items-center justify-center rounded opacity-40 hover:opacity-100 transition-opacity"
              style="color:white" title="Go to end">
        <svg width="22" height="22" viewBox="0 0 24 24" fill="currentColor">
          <path d="M6 18l8.5-6L6 6v12z"/><rect x="16" y="6" width="2" height="12"/>
        </svg>
      </button>
    </div>

      <!-- Timecode — right -->
      <span class="absolute right-3 font-mono tabular-nums" style="font-size:12px; color:white; opacity:0.7">
        {fmt(currentTime)}{#if duration} / {fmt(duration)}{/if}
      </span>
      </div><!-- /controls row -->

    </div><!-- /left column -->

    <!-- Right column: VU meter spanning full 176px height -->
    <div class="relative shrink-0 mr-3 mt-1.5 mb-1" style="width:48px">
      <canvas bind:this={vuCanvas} width="96" height="352"
              style="width:48px; height:100%; display:block; border-radius:4px"
      ></canvas>
      <div class="absolute inset-0 pointer-events-none" style="font:8px monospace">
        {#each [[0,'0'],[10,'-6'],[20,'-12'],[40,'-24'],[80,'-48']] as [pct, lbl]}
          <span class="absolute right-0.5 leading-none"
                style="top:{pct}%; transform:translateY(-50%); color:rgba(255,255,255,0.45)">{lbl}</span>
        {/each}
      </div>
    </div>

  </div><!-- /main timeline row -->

</div>
