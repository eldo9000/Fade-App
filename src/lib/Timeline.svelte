<script>
  import { convertFileSrc, invoke } from '@tauri-apps/api/core';
  import { listen } from '@tauri-apps/api/event';

  let { item, duration = null, options = $bindable(null), mediaEl = null, onscrubstart = null, vizExpanded = $bindable(false), mediaReady = false, waveformReady = false, spectrogramReady = false, filmstripReady = false } = $props();

  // ── Media element ─────────────────────────────────────────────────────────
  // When `mediaEl` prop is supplied (e.g. the preview <video>), Timeline drives
  // it directly. Otherwise it creates an internal Audio object (audio-only files).
  let audioEl        = $state(null);
  let isPlaying      = $state(false);
  let currentTime    = $state(0);
  let _prevAudio     = null;
  let _ownedInternal = false;

  $effect(() => {
    const it = item;
    const external = mediaEl;
    const mediaReady_ = mediaReady;

    // Teardown previous
    if (_prevAudio && _ownedInternal) { _prevAudio.pause(); _prevAudio.src = ''; }
    _prevAudio = null; _ownedInternal = false;
    _teardownGraph();
    isPlaying = false; currentTime = 0; audioEl = null;
    if (!it || !mediaReady_) return;  // gate: don't create Audio until parent says ready

    let el;
    if (external) {
      el = external;
      _ownedInternal = false;
    } else {
      try { el = new Audio(convertFileSrc(it.path)); el.preload = 'auto'; }
      catch { return; }
      _ownedInternal = true;
    }

    const onTime  = () => { currentTime = el.currentTime; };
    const onEnded = () => { isPlaying = false; currentTime = 0; };
    const onPlay  = () => { isPlaying = true; };
    const onPause = () => { isPlaying = false; };
    el.addEventListener('timeupdate', onTime);
    el.addEventListener('ended',      onEnded);
    el.addEventListener('play',       onPlay);
    el.addEventListener('pause',      onPause);

    // If the external element already has a currentTime (e.g. user scrubbed
    // before Timeline mounted), pick it up immediately.
    if (!Number.isNaN(el.currentTime)) currentTime = el.currentTime;

    audioEl = el; _prevAudio = el;
    _initCtx(); // Phase 1: pre-warm context + nodes (no audio element touch)

    return () => {
      el.removeEventListener('timeupdate', onTime);
      el.removeEventListener('ended',      onEnded);
      el.removeEventListener('play',       onPlay);
      el.removeEventListener('pause',      onPause);
      if (_ownedInternal) { el.pause(); el.src = ''; }
      _prevAudio = null; _ownedInternal = false;
      _teardownGraph();
    };
  });

  // ── Web Audio graph ───────────────────────────────────────────────────────
  // Architecture: audio plays on the NATIVE media path (audioEl → speakers),
  // NOT through the Web Audio context. This eliminates Web Audio hardware-buffer
  // latency (~25-100ms) from play/pause/stop — changes are heard immediately.
  //
  // For analyser data we use one of two strategies (tried in order):
  //   1. captureStream() — taps a copy of the native output; zero impact on
  //      playback latency. Available in Chrome/Electron; NOT in WKWebView/Tauri.
  //   2. Shadow element — a second Audio at volume=0 routed through
  //      createMediaElementSource → analysers. Its Web Audio hardware-buffer
  //      latency only affects that silent copy, not the main element, so the
  //      user hears no delay. We keep its currentTime in sync with the main
  //      element so the analyser data matches what is being heard.
  let _audioCtx     = null;
  let _analyserL    = null;
  let _analyserR    = null;
  let _splitter     = null;
  let _srcConnected = false;
  let _shadowEl     = null; // silent clone feeding analysers (fallback path)

  function _teardownGraph() {
    if (_rafId) { cancelAnimationFrame(_rafId); _rafId = null; }
    if (_shadowEl) { _shadowEl.pause(); _shadowEl.src = ''; _shadowEl = null; }
    if (_audioCtx) { _audioCtx.close().catch(() => {}); _audioCtx = null; }
    _analyserL = _analyserR = _splitter = null;
    _srcConnected = false;
    _clipL = _clipR = false; _clipHoldL = _clipHoldR = 0;
  }

  // Phase 1 — create analyser context + nodes, no audio element touch.
  function _initCtx() {
    if (_audioCtx) return;
    try {
      const ctx = new AudioContext({ latencyHint: 'interactive' });
      const splitter = ctx.createChannelSplitter(2);
      const aL = ctx.createAnalyser();
      const aR = ctx.createAnalyser();
      aL.fftSize = aR.fftSize = 2048;
      aL.smoothingTimeConstant = aR.smoothingTimeConstant = 0.75;
      splitter.connect(aL, 0);
      splitter.connect(aR, 1);
      _audioCtx = ctx; _splitter = splitter; _analyserL = aL; _analyserR = aR;
      ctx.resume().catch(() => {});
    } catch (e) { console.error('AudioContext init failed:', e); }
  }

  // Phase 2 — wire a signal source into the analyser graph.
  // Try captureStream() first (zero latency impact). Fall back to a silent
  // shadow Audio element routed through createMediaElementSource so the
  // analysers always have a real signal regardless of WKWebView limitations.
  function _connectSource() {
    if (_srcConnected || !audioEl || !_audioCtx || !_splitter) return;
    try {
      if (typeof audioEl.captureStream === 'function') {
        // Best path: tap a copy of the native output stream.
        const src = _audioCtx.createMediaStreamSource(audioEl.captureStream());
        src.connect(_splitter);
        _srcConnected = true;
      } else {
        // Fallback: silent shadow element through Web Audio — analysers only.
        // The main audioEl keeps playing natively so there is no added latency.
        const shadow = new Audio(audioEl.src);
        shadow.volume = 0;
        shadow.currentTime = audioEl.currentTime;
        shadow.loop = audioEl.loop;
        const src = _audioCtx.createMediaElementSource(shadow);
        src.connect(_splitter);
        // GainNode keeps shadow silent even after createMediaElementSource
        // (createMediaElementSource by itself routes to ctx.destination too).
        const mute = _audioCtx.createGain();
        mute.gain.value = 0;
        src.connect(mute);
        mute.connect(_audioCtx.destination);
        _shadowEl = shadow;
        _srcConnected = true;
      }
    } catch (e) { console.error('connectSource failed:', e); }
  }

  // Keep the shadow element in sync with the main element.

  // ── Playback controls ─────────────────────────────────────────────────────

  // De-click: time-based linear volume ramp via chained setTimeout(fn,0).
  // Each tick fires as soon as possible (~1 ms), and performance.now() drives
  // the volume so the curve is accurate to actual elapsed time regardless of
  // timer jitter.  durationMs controls the full ramp length (10 ms fade-in,
  // 20 ms fade-out).
  //
  // Interruption safety: _declickId increments on every new ramp.  Any in-
  // flight tick that sees a stale id simply returns without touching the
  // volume or calling onDone — so no snap occurs.  The new ramp reads
  // el.volume at call-time and smoothly continues from wherever the old ramp
  // left off.  onDone (e.g. el.pause()) only fires when *that* ramp reaches
  // its target naturally; a superseded ramp never calls its onDone, which is
  // correct (e.g. a play() that interrupts a pause() fade-out should not
  // call el.pause()).
  let _declickId     = 0;
  let _declickActive = false; // true while a ramp is in-flight; blocks envelope from overriding volume

  function _declick(to, durationMs, onDone) {
    const el = audioEl;
    if (!el) { onDone?.(); return; }
    const from = el.volume;
    const id = ++_declickId;
    _declickActive = true;
    const start = performance.now();
    const tick = () => {
      if (_declickId !== id || audioEl !== el) { _declickActive = false; return; }
      const t = Math.min(1, (performance.now() - start) / durationMs);
      el.volume = from + (to - from) * t;
      if (t < 1) setTimeout(tick, 0);
      else { _declickActive = false; onDone?.(); }
    };
    setTimeout(tick, 0);
  }

  // Compute the fade-envelope amplitude [0,1] for a given playback time.
  // Uses logarithmic curves so the fade sounds perceptually linear in loudness.
  function _computeFadeEnvelope(t) {
    const trimStart = options?.trim_start ?? 0;
    const trimEnd   = options?.trim_end   ?? duration ?? 0;
    const fadeIn    = options?.fade_in    ?? 0;
    const fadeOut   = options?.fade_out   ?? 0;
    let vol = 1;
    if (fadeIn > 0 && t < trimStart + fadeIn) {
      const p = Math.max(0, (t - trimStart) / fadeIn); // linear 0→1
      vol = Math.min(vol, p * p);                       // square for perceptual linearity
    }
    if (fadeOut > 0 && t > trimEnd - fadeOut) {
      const p = Math.max(0, (trimEnd - t) / fadeOut);
      vol = Math.min(vol, p * p);
    }
    return vol;
  }

  // Apply fade envelope to audio volume and video brightness.
  // Called from the RAF loop (while playing) and on timeupdate (while scrubbing).
  function _applyEnvelope() {
    const t = audioEl?.currentTime ?? currentTime;
    const vol = _computeFadeEnvelope(t);
    // Audio: only override when de-click is not active (de-click wins briefly on play/pause)
    if (audioEl && !_declickActive && isPlaying) audioEl.volume = vol;
    // Video: always apply brightness so scrubbing into fades shows the effect
    if (mediaEl) mediaEl.style.filter = vol < 0.999 ? `brightness(${vol.toFixed(4)})` : '';
  }

  function togglePlay() {
    if (!audioEl) return;
    if (isPlaying) pause(); else play();
  }

  function seekTo(secs) {
    currentTime = Math.max(0, Math.min(duration ?? 0, secs));
    if (audioEl) audioEl.currentTime = currentTime;
    if (_shadowEl) _shadowEl.currentTime = currentTime;
  }

  let loopEnabled      = $state(false);
  let startHovered     = $state(false);
  let endHovered       = $state(false);
  let fadeInHovered    = $state(false);
  let fadeOutHovered   = $state(false);

  function play() {
    if (!audioEl || isPlaying) return;
    if (!_audioCtx) _initCtx();
    _connectSource();
    _audioCtx?.resume();
    isPlaying = true;
    audioEl.volume = 0;
    audioEl.play().catch(() => { isPlaying = false; });
    if (_shadowEl) { _shadowEl.currentTime = audioEl.currentTime; _shadowEl.play().catch(() => {}); }
    _declick(1, 25, null);
  }

  function pause() {
    if (!audioEl) return;
    isPlaying = false;
    const el = audioEl;
    const sh = _shadowEl;
    _declick(0, 25, () => { el.pause(); el.volume = 1; sh?.pause(); if (mediaEl) mediaEl.style.filter = ''; });
  }

  function stop() {
    if (!audioEl) return;
    isPlaying = false;
    const el = audioEl;
    const sh = _shadowEl;
    _declick(0, 25, () => { el.pause(); el.volume = 1; sh?.pause(); seekTo(options?.trim_start ?? 0); if (mediaEl) mediaEl.style.filter = ''; });
  }

  // Seek with de-click: always mute → seek → fade back in so the initial click
  // is inaudible even when audioEl.play() (called by _beginScrub) hasn't resolved
  // yet (play() is async, so audioEl.paused may still be true at call time).
  function seekWithDeclick(secs) {
    if (!audioEl) return;
    audioEl.volume = 0;
    seekTo(secs);
    _declick(1, 25, null);
  }

  $effect(() => {
    if (audioEl) audioEl.loop = loopEnabled;
  });

  // ── Canvas refs ───────────────────────────────────────────────────────────
  let lissajousCanvas    = $state(null);
  let oscilloscopeCanvas = $state(null);
  let spectrumCanvas     = $state(null);
  let vuCanvas           = $state(null);

  // VU meter peak-hold state (plain vars — not reactive)
  let _peakL = -60, _peakR = -60;
  let _peakHoldL = 0, _peakHoldR = 0;
  let _clipL = false, _clipR = false;
  let _clipHoldL = 0, _clipHoldR = 0;
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
      ctx.strokeStyle = i === 2 ? 'rgba(255,255,255,0.52)' : 'rgba(255,255,255,0.24)';
      ctx.beginPath(); ctx.moveTo(0, y); ctx.lineTo(w, y); ctx.stroke();
    }
    for (let i = 0; i <= 8; i++) {
      const x = (i / 8) * w;
      ctx.strokeStyle = 'rgba(255,255,255,0.24)';
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
      ctx.strokeStyle = i === 2 ? 'rgba(255,255,255,0.26)' : 'rgba(255,255,255,0.12)';
      ctx.beginPath(); ctx.moveTo(0, y); ctx.lineTo(w, y); ctx.stroke();
    }
    for (let i = 0; i <= 8; i++) {
      const x = (i / 8) * w;
      ctx.strokeStyle = 'rgba(255,255,255,0.12)';
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

    // Clip detection — light up at ≥ −0.5 dBFS, hold ~2 s
    if (dbL >= -0.5) { _clipL = true; _clipHoldL = 120; }
    else if (_clipHoldL > 0) _clipHoldL--;
    else _clipL = false;
    if (dbR >= -0.5) { _clipR = true; _clipHoldR = 120; }
    else if (_clipHoldR > 0) _clipHoldR--;
    else _clipR = false;

    const CLIP_H = 16; // canvas px reserved at top for clip indicators
    const DB_MIN = -60, DB_MAX = 0;
    const toY = (db) => CLIP_H + (1 - (db - DB_MIN) / (DB_MAX - DB_MIN)) * (h - CLIP_H);

    // Bars: half width, packed left so dB labels on right have clear room
    const L_X = 4, R_X = 28, BAR_W = 19;

    // Clip indicators at top of each stripe
    ctx.fillStyle = _clipL ? '#ef4444' : '#220808';
    ctx.fillRect(L_X, 2, BAR_W, CLIP_H - 4);
    ctx.fillStyle = _clipR ? '#ef4444' : '#220808';
    ctx.fillRect(R_X, 2, BAR_W, CLIP_H - 4);

    // Red (loud) → yellow → green (quiet) gradient (meter area only)
    const meterRange = h - CLIP_H;
    const grad = ctx.createLinearGradient(0, CLIP_H, 0, h);
    grad.addColorStop(0,                                           '#ef4444');
    grad.addColorStop((toY(-6)  - CLIP_H) / meterRange,           '#ef4444');
    grad.addColorStop((toY(-6)  - CLIP_H) / meterRange + 0.001,   '#eab308');
    grad.addColorStop((toY(-12) - CLIP_H) / meterRange,           '#eab308');
    grad.addColorStop((toY(-12) - CLIP_H) / meterRange + 0.001,   '#22c55e');
    grad.addColorStop(1,                                           '#14532d');

    function drawBar(db, peak, x) {
      ctx.fillStyle = '#111';
      ctx.fillRect(x, CLIP_H, BAR_W, h - CLIP_H);
      const lY = toY(db);
      if (lY < h) { ctx.fillStyle = grad; ctx.fillRect(x, lY, BAR_W, h - lY); }
      if (peak > DB_MIN) {
        const py = Math.max(CLIP_H, toY(peak) - 1);
        ctx.fillStyle = peak > -6 ? '#fca5a5' : peak > -12 ? '#fde047' : '#86efac';
        ctx.fillRect(x, py, BAR_W, 2);
      }
    }

    drawBar(dbL, _peakL, L_X);
    drawBar(dbR, _peakR, R_X);

    // Tick lines across both bars at key dB positions
    ctx.fillStyle = 'rgba(255,255,255,0.18)';
    [-6, -12, -18, -24, -48].forEach(db => {
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
    _applyEnvelope();
    _rafId = requestAnimationFrame(_renderLoop);
  }

  $effect(() => {
    // Run the render loop whenever the visualiser panel is visible OR audio is
    // playing — so grids/axes appear immediately on expand even before playback.
    if (isPlaying || vizExpanded) {
      if (!_rafId) _rafId = requestAnimationFrame(_renderLoop);
    } else {
      if (_rafId) { cancelAnimationFrame(_rafId); _rafId = null; }
    }
    return () => { if (_rafId) { cancelAnimationFrame(_rafId); _rafId = null; } };
  });

  // ── Static waveform / spectrogram / filmstrip (loaded via ffmpeg) ─────────
  /** @type {{ amplitudes: number[], hues: number[] } | null} */
  let waveformData    = $state(null);
  let spectrogramData = $state(null);
  /** @type {string[]} */
  let filmstripFrames = $state([]);
  let mediaLoading    = $state(false);
  let _capturedId     = null;

  $effect(() => {
    const it = item;
    const go = waveformReady;
    waveformData = null;
    if (!it || !go) return;
    const id = it.id; _capturedId = id; mediaLoading = true;
    invoke('get_waveform', { path: it.path })
      .then(d => { if (_capturedId === id) { waveformData = /** @type {any} */ (d); mediaLoading = false; } })
      .catch(e => { console.error('get_waveform failed:', e); if (_capturedId === id) mediaLoading = false; });
  });

  $effect(() => {
    const it = item;
    const go = spectrogramReady;
    spectrogramData = null;
    if (!it || !go) return;
    const id = it.id;
    invoke('get_spectrogram', { path: it.path })
      .then(b64 => { if (_capturedId === id) spectrogramData = b64; })
      .catch(() => {});
  });

  $effect(() => {
    const it = item;
    const dur = duration;
    const go = filmstripReady;
    filmstripFrames = [];
    if (!it || !go || !dur || it.mediaType !== 'video') return;

    const myId = it.id;
    const COUNT = 20;
    // Pre-allocate slots so the strip renders a skeleton immediately
    filmstripFrames = new Array(COUNT).fill(null);

    // Subscribe before invoking so no frames are missed
    let unlistenFn = null;
    listen('filmstrip-frame', (ev) => {
      const { id, index, data } = ev.payload;
      if (id !== myId) return; // stale item
      filmstripFrames[index] = data;
    }).then(fn => { unlistenFn = fn; });

    // Fire-and-forget — Rust thread emits events as each frame finishes
    invoke('get_filmstrip', { path: it.path, id: myId, count: COUNT, duration: dur })
      .catch(() => {});

    return () => { unlistenFn?.(); };
  });

  // Apply envelope whenever currentTime changes (covers scrubbing while paused).
  // During playback the RAF loop calls _applyEnvelope each frame instead.
  $effect(() => {
    void currentTime; // track dependency
    if (!isPlaying) _applyEnvelope();
  });

  // ── Derived fractions ─────────────────────────────────────────────────────
  let startFrac = $derived(
    duration && options?.trim_start != null
      ? Math.max(0, Math.min(1, options.trim_start / duration)) : 0);
  let endFrac = $derived(
    duration && options?.trim_end != null
      ? Math.max(0, Math.min(1, options.trim_end / duration)) : 1);
  let playFrac = $derived(duration ? Math.max(0, Math.min(1, currentTime / duration)) : 0);

  // Fade handle fractions — clamped so they stay inside [startFrac, endFrac].
  let fadeInFrac = $derived(
    duration ? Math.min(endFrac, Math.max(startFrac,
      ((options?.trim_start ?? 0) + (options?.fade_in ?? 0)) / duration)) : startFrac);
  let fadeOutFrac = $derived(
    duration ? Math.max(startFrac, Math.min(endFrac,
      ((options?.trim_end ?? duration ?? 0) - (options?.fade_out ?? 0)) / duration)) : endFrac);

  // ── Drag ──────────────────────────────────────────────────────────────────
  let trackEl       = $state(null);
  let filmstripEl   = $state(null);
  let dragging      = $state(null); // 'start' | 'end' | 'playhead'
  let _dragEl       = null; // element the current drag started on
  let _wasPlayingBeforeDrag = false;

  function getFrac(e) {
    const el = _dragEl ?? trackEl;
    if (!el) return 0;
    const r = el.getBoundingClientRect();
    return Math.max(0, Math.min(1, (e.clientX - r.left) / r.width));
  }
  function fracToSecs(f) { return f * (duration ?? 0); }

  // While scrubbing, keep playback active so the Web Audio analysers have
  // a live signal for the visualizers. Restore paused state on mouseup.
  function _beginScrub() {
    if (!audioEl) return;
    _wasPlayingBeforeDrag = isPlaying;
    if (!isPlaying) {
      if (!_audioCtx) _initCtx();
      _connectSource();
      _audioCtx?.resume();
      audioEl.play().catch(() => {});
      if (_shadowEl) { _shadowEl.currentTime = audioEl.currentTime; _shadowEl.play().catch(() => {}); }
    }
  }
  function _endScrub() {
    if (audioEl && !_wasPlayingBeforeDrag && !audioEl.paused) {
      const sh = _shadowEl;
      _declick(0, 25, () => { audioEl?.pause(); if (audioEl) audioEl.volume = 1; sh?.pause(); });
    }
    _wasPlayingBeforeDrag = false;
  }

  function onTrackDown(e) {
    if (!duration) return;
    _dragEl = trackEl;
    onscrubstart?.();
    dragging = 'playhead';
    _beginScrub();
    seekWithDeclick(fracToSecs(getFrac(e)));
  }
  function onFilmstripDown(e) {
    if (!duration) return;
    _dragEl = filmstripEl;
    onscrubstart?.();
    dragging = 'playhead';
    _beginScrub();
    seekWithDeclick(fracToSecs(getFrac(e)));
  }
  function onWindowMouseMove(e) {
    if (!dragging || !duration) return;
    const f = getFrac(e);
    if      (dragging === 'start')    options.trim_start = fracToSecs(Math.min(f, endFrac - 1 / duration));
    else if (dragging === 'end')      options.trim_end   = fracToSecs(Math.max(f, startFrac + 1 / duration));
    else if (dragging === 'fade_in')  options.fade_in    = Math.max(0, fracToSecs(Math.min(f, fadeOutFrac)) - (options.trim_start ?? 0));
    else if (dragging === 'fade_out') options.fade_out   = Math.max(0, (options.trim_end ?? duration ?? 0) - fracToSecs(Math.max(f, fadeInFrac)));
    else                              seekTo(fracToSecs(f));
  }
  function onWindowMouseUp() {
    if (dragging === 'playhead') _endScrub();
    dragging = null;
    _dragEl = null;
  }

  // ── Helpers ───────────────────────────────────────────────────────────────
  function fmt(secs) {
    if (secs == null || secs < 0) return '—';
    const m = Math.floor(secs / 60);
    const s = (secs % 60).toFixed(1);
    return `${m}:${s.padStart(4, '0')}`;
  }

  function fmtTC(secs) {
    if (secs == null || secs < 0) return '00:00:00';
    const h = Math.floor(secs / 3600);
    const m = Math.floor((secs % 3600) / 60);
    const s = Math.floor(secs % 60);
    return `${String(h).padStart(2,'0')}:${String(m).padStart(2,'0')}:${String(s).padStart(2,'0')}`;
  }

  const showTrim   = $derived(options != null && duration != null);
</script>

<svelte:window onmousemove={onWindowMouseMove} onmouseup={onWindowMouseUp}
  onkeydown={(e) => {
    if (e.key === ' ' && !['INPUT','TEXTAREA','SELECT'].includes(e.target?.tagName ?? '')) {
      e.preventDefault();
      _audioCtx?.resume(); // wake context ahead of togglePlay
      togglePlay();
    }
  }}
/>

<div class="shrink-0 border-t border-[var(--border)] flex flex-col select-none"
     style="background:#0a0a0a">

  <!-- ── Visualiser — collapsible header ──────────────────────────────────── -->
  <div class="w-full relative flex items-center shrink-0 select-none"
       style="height:30px; border-bottom:1px solid rgba(255,255,255,0.07)">
    <!-- Chevron centred over the play button — points up when closed (inviting click), down when open -->
    <button
      onclick={() => vizExpanded = !vizExpanded}
      class="absolute left-1/2 -translate-x-1/2 px-10 py-1 rounded
             bg-white/[0.04] border border-white/[0.07]
             hover:bg-white/[0.12] hover:border-white/20 transition-colors"
      aria-label={vizExpanded ? 'Collapse visualisers' : 'Expand visualisers'}
    >
      <svg width="20" height="14" viewBox="0 0 20 14" fill="none"
           stroke="rgba(255,255,255,0.4)" stroke-width="2"
           stroke-linecap="round" stroke-linejoin="round">
        {#if vizExpanded}
          <!-- open → chevrons point down (click to collapse) -->
          <path d="M1 1 L10 7 L19 1"/>
          <path d="M1 7 L10 13 L19 7"/>
        {:else}
          <!-- closed → chevrons point up (click to expand) -->
          <path d="M1 13 L10 7 L19 13"/>
          <path d="M1 7  L10 1 L19 7"/>
        {/if}
      </svg>
    </button>
  </div>

  <!-- ── Full-height flex: left content | VU meter spanning all rows ───────── -->
  <div class="flex min-h-0">

    <!-- Left column: optional expanded viz + waveform + controls -->
    <div class="flex-1 min-w-0 flex flex-col">

      {#if vizExpanded}
        <!-- Spectrogram (224px) — px-3 aligns with waveform mx-3 -->
        {#if spectrogramData}
          <div class="shrink-0 px-3 pt-2" style="height:224px">
            <img src="data:image/png;base64,{spectrogramData}" alt="Spectrogram"
                 class="w-full h-full object-fill rounded" style="display:block" />
          </div>
        {/if}

        <!-- Viz row: lissajous | oscilloscope (1x) | spectrum (2x) -->
        <div class="shrink-0 flex gap-2 px-3 pt-2 pb-2" style="height:120px">
          <canvas bind:this={lissajousCanvas} width="216" height="216"
                  style="width:108px; height:108px; border-radius:4px; background:#000; flex-shrink:0; display:block"
          ></canvas>
          <!-- Oscilloscope: half the width of spectrum -->
          <div style="flex:1 1 0%; min-width:0; height:108px; position:relative">
            <canvas bind:this={oscilloscopeCanvas} width="512" height="216"
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
          <!-- Spectrum: double width of oscilloscope -->
          <div style="flex:2 1 0%; min-width:0; height:108px; position:relative">
            <canvas bind:this={spectrumCanvas} width="1024" height="216"
                    style="width:100%; height:108px; border-radius:4px; display:block; background:#000"
            ></canvas>
            <div class="absolute inset-0 pointer-events-none rounded overflow-hidden">
              {#each [[25,'100'],[46,'500'],[56,'1k'],[79,'5k'],[89,'10k']] as [pct, lbl]}
                <div class="absolute top-0 bottom-0" style="left:{pct}%; border-left:1px solid rgba(255,255,255,0.36)"></div>
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

      <!-- Filmstrip (video only, above waveform) -->
      {#if item?.mediaType === 'video' && filmstripFrames.length > 0}
        <!-- svelte-ignore a11y_no_static_element_interactions -->
        <div bind:this={filmstripEl}
             class="shrink-0 relative mx-3 mb-1 mt-1.5 rounded overflow-hidden cursor-crosshair"
             style="height:68px; background:#111"
             onmousedown={onFilmstripDown}>
          <!-- Frame strip -->
          <div class="absolute inset-0 flex gap-px">
            {#each filmstripFrames as frame}
              {#if frame}
                <!-- svelte-ignore a11y_missing_attribute -->
                <img src="data:image/jpeg;base64,{frame}"
                     class="h-full object-cover min-w-0"
                     style="flex:1 1 0%"
                     draggable="false" />
              {:else}
                <div class="h-full min-w-0" style="flex:1 1 0%; background:#1c1c1c"></div>
              {/if}
            {/each}
          </div>
          <!-- Playhead -->
          {#if duration}
            <div class="absolute inset-y-0 z-10 pointer-events-none"
                 style="left:{playFrac * 100}%; transform:translateX(-50%)">
              <div class="absolute top-0 bottom-0 left-1/2 -translate-x-px w-[2px]"
                   style="background:#60a5fa; opacity:0.85"></div>
              <div class="absolute left-1/2 -translate-x-1/2 w-2.5 h-2.5 rounded-full"
                   style="bottom:-5px; background:#60a5fa; box-shadow:0 0 6px #60a5fa"></div>
            </div>
          {/if}
        </div>
      {/if}

      <!-- Waveform track + controls (fixed 176px) -->
      <div class="shrink-0 flex flex-col" style="height:176px">

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
      {:else if item}
        <div class="absolute inset-0 flex items-center justify-center">
          <span style="font-size:13px; font-weight:500; color:rgba(255,255,255,0.25)">Loading</span>
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
      <!-- Fade-in curve overlay -->
      {#if (options?.fade_in ?? 0) > 0}
        <div class="absolute inset-y-0 pointer-events-none overflow-hidden"
             style="left:{startFrac * 100}%; width:{(fadeInFrac - startFrac) * 100}%">
          <svg class="w-full h-full" preserveAspectRatio="none" viewBox="0 0 100 100" xmlns="http://www.w3.org/2000/svg">
            <!-- Wedge: full-height at crop edge, tapers to a point at fade handle -->
            <path d="M 0,0 C 20,0 100,50 100,50 C 100,50 20,100 0,100 Z" fill="rgba(0,0,0,0.45)"/>
          </svg>
        </div>
      {/if}
      <!-- Fade-out curve overlay -->
      {#if (options?.fade_out ?? 0) > 0}
        <div class="absolute inset-y-0 pointer-events-none overflow-hidden"
             style="left:{fadeOutFrac * 100}%; width:{(endFrac - fadeOutFrac) * 100}%">
          <svg class="w-full h-full" preserveAspectRatio="none" viewBox="0 0 100 100" xmlns="http://www.w3.org/2000/svg">
            <path d="M 100,0 C 80,0 0,50 0,50 C 0,50 80,100 100,100 Z" fill="rgba(0,0,0,0.45)"/>
          </svg>
        </div>
      {/if}
      <!-- Fade-in triangle handle (sits at fadeInFrac, top of track) -->
      <!-- svelte-ignore a11y_no_static_element_interactions -->
      <div class="absolute z-25 cursor-ew-resize"
           style="left:{fadeInFrac * 100}%; top:0; transform:translateX(-50%)"
           onmouseenter={() => fadeInHovered = true}
           onmouseleave={() => fadeInHovered = false}
           onmousedown={(e) => { e.stopPropagation(); dragging = 'fade_in'; }}>
        <svg width="14" height="10" viewBox="0 0 14 10" xmlns="http://www.w3.org/2000/svg">
          <polygon points="7,10 0,0 14,0"
                   fill="{(dragging==='fade_in'||fadeInHovered) ? 'rgba(251,191,36,1)' : 'rgba(251,191,36,0.65)'}"/>
        </svg>
      </div>
      <!-- Fade-out triangle handle (sits at fadeOutFrac, top of track) -->
      <!-- svelte-ignore a11y_no_static_element_interactions -->
      <div class="absolute z-25 cursor-ew-resize"
           style="left:{fadeOutFrac * 100}%; top:0; transform:translateX(-50%)"
           onmouseenter={() => fadeOutHovered = true}
           onmouseleave={() => fadeOutHovered = false}
           onmousedown={(e) => { e.stopPropagation(); dragging = 'fade_out'; }}>
        <svg width="14" height="10" viewBox="0 0 14 10" xmlns="http://www.w3.org/2000/svg">
          <polygon points="7,10 0,0 14,0"
                   fill="{(dragging==='fade_out'||fadeOutHovered) ? 'rgba(251,191,36,1)' : 'rgba(251,191,36,0.65)'}"/>
        </svg>
      </div>
      <!-- Left handle -->
      <!-- svelte-ignore a11y_no_static_element_interactions -->
      <div class="absolute inset-y-0 z-20 flex items-center justify-center cursor-ew-resize"
           style="left:calc({startFrac * 100}% - 7px); width:14px"
           onmouseenter={() => startHovered = true}
           onmouseleave={() => startHovered = false}
           onmousedown={(e) => { e.stopPropagation(); dragging = 'start'; }}>
        <div class="absolute inset-y-0 w-[2px] rounded-full"
             style="background:{(dragging==='start'||startHovered) ? 'rgba(255,255,255,0.85)' : 'rgba(255,255,255,0.25)'}; transition:background 0.12s"></div>
        <div class="relative z-10 flex items-center gap-px px-[3px] py-[5px] rounded-sm"
             style="background:{(dragging==='start'||startHovered) ? 'rgba(255,255,255,0.22)' : 'rgba(255,255,255,0.06)'}; border:1px solid {(dragging==='start'||startHovered) ? 'rgba(255,255,255,0.45)' : 'rgba(255,255,255,0.12)'}; transition:background 0.12s, border-color 0.12s">
          <svg width="3" height="7" viewBox="0 0 3 7" style="fill:{(dragging==='start'||startHovered) ? 'rgba(255,255,255,0.95)' : 'rgba(255,255,255,0.35)'}; transition:fill 0.12s"><path d="M3 0 L0 3.5 L3 7 Z"/></svg>
          <svg width="3" height="7" viewBox="0 0 3 7" style="fill:{(dragging==='start'||startHovered) ? 'rgba(255,255,255,0.95)' : 'rgba(255,255,255,0.35)'}; transition:fill 0.12s"><path d="M0 0 L3 3.5 L0 7 Z"/></svg>
        </div>
      </div>
      <!-- Right handle -->
      <!-- svelte-ignore a11y_no_static_element_interactions -->
      <div class="absolute inset-y-0 z-20 flex items-center justify-center cursor-ew-resize"
           style="left:calc({endFrac * 100}% - 7px); width:14px"
           onmouseenter={() => endHovered = true}
           onmouseleave={() => endHovered = false}
           onmousedown={(e) => { e.stopPropagation(); dragging = 'end'; }}>
        <div class="absolute inset-y-0 w-[2px] rounded-full"
             style="background:{(dragging==='end'||endHovered) ? 'rgba(255,255,255,0.85)' : 'rgba(255,255,255,0.25)'}; transition:background 0.12s"></div>
        <div class="relative z-10 flex items-center gap-px px-[3px] py-[5px] rounded-sm"
             style="background:{(dragging==='end'||endHovered) ? 'rgba(255,255,255,0.22)' : 'rgba(255,255,255,0.06)'}; border:1px solid {(dragging==='end'||endHovered) ? 'rgba(255,255,255,0.45)' : 'rgba(255,255,255,0.12)'}; transition:background 0.12s, border-color 0.12s">
          <svg width="3" height="7" viewBox="0 0 3 7" style="fill:{(dragging==='end'||endHovered) ? 'rgba(255,255,255,0.95)' : 'rgba(255,255,255,0.35)'}; transition:fill 0.12s"><path d="M3 0 L0 3.5 L3 7 Z"/></svg>
          <svg width="3" height="7" viewBox="0 0 3 7" style="fill:{(dragging==='end'||endHovered) ? 'rgba(255,255,255,0.95)' : 'rgba(255,255,255,0.35)'}; transition:fill 0.12s"><path d="M0 0 L3 3.5 L0 7 Z"/></svg>
        </div>
      </div>
    {/if}

    <!-- Playhead -->
    {#if duration}
      <!-- svelte-ignore a11y_no_static_element_interactions -->
      <div class="absolute inset-y-0 z-30 cursor-ew-resize"
           style="left:{playFrac * 100}%; transform:translateX(-50%)"
           onmousedown={(e) => { e.stopPropagation(); dragging = 'playhead'; _beginScrub(); }}>
        <div class="absolute left-1/2 -translate-x-1/2 w-2.5 h-2.5 rounded-full"
             style="top:-5px; background:#60a5fa; box-shadow:0 0 7px #60a5fa"></div>
        <div class="absolute top-0 bottom-0 left-1/2 -translate-x-px w-px"
             style="background:#60a5fa"></div>
      </div>
    {/if}
      </div><!-- /track -->

      <!-- Controls row -->
      <div class="relative shrink-0" style="height:44px">

    <!-- Playback buttons — left -->
    <div class="absolute left-3 top-0 bottom-0 flex items-center gap-1">
      <!-- Start -->
      <button onclick={() => seekWithDeclick(options?.trim_start ?? 0)}
              class="flex items-center justify-center rounded hover:brightness-125"
              style="width:30px; height:26px; color:rgba(255,255,255,0.6); background:rgba(255,255,255,0.07); border:1px solid rgba(255,255,255,0.1)"
              title="Go to start">
        <svg width="13" height="13" viewBox="0 0 24 24" fill="currentColor">
          <path d="M6 6h2v12H6zm3.5 6 8.5 6V6z"/>
        </svg>
      </button>
      <!-- Play / Pause toggle -->
      <button onpointerdown={() => _audioCtx?.resume()} onclick={togglePlay}
              class="flex items-center justify-center rounded hover:brightness-110"
              style="width:30px; height:26px; color:white; background:{isPlaying ? '#2563eb' : 'transparent'}; border:1px solid {isPlaying ? '#3b82f6' : '#3b82f6'}"
              title="{isPlaying ? 'Pause' : 'Play'} (Space)">
        {#if isPlaying}
          <svg width="13" height="13" viewBox="0 0 24 24" fill="currentColor">
            <rect x="6" y="4" width="4" height="16"/><rect x="14" y="4" width="4" height="16"/>
          </svg>
        {:else}
          <svg width="13" height="13" viewBox="0 0 24 24" fill="currentColor">
            <path d="M8 5v14l11-7z"/>
          </svg>
        {/if}
      </button>
      <!-- Stop -->
      <button onclick={stop}
              class="flex items-center justify-center rounded hover:brightness-125"
              style="width:30px; height:26px; color:rgba(255,255,255,0.6); background:rgba(255,255,255,0.07); border:1px solid rgba(255,255,255,0.1)"
              title="Stop">
        <svg width="13" height="13" viewBox="0 0 24 24" fill="currentColor">
          <rect x="6" y="6" width="12" height="12"/>
        </svg>
      </button>
      <!-- End -->
      <button onclick={() => seekWithDeclick(options?.trim_end ?? duration ?? 0)}
              class="flex items-center justify-center rounded hover:brightness-125"
              style="width:30px; height:26px; color:rgba(255,255,255,0.6); background:rgba(255,255,255,0.07); border:1px solid rgba(255,255,255,0.1)"
              title="Go to end">
        <svg width="13" height="13" viewBox="0 0 24 24" fill="currentColor">
          <path d="M6 18l8.5-6L6 6v12z"/><rect x="16" y="6" width="2" height="12"/>
        </svg>
      </button>
      <!-- Loop -->
      <button onclick={() => loopEnabled = !loopEnabled}
              class="flex items-center justify-center rounded hover:brightness-125"
              style="width:30px; height:26px; color:{loopEnabled ? 'white' : 'rgba(255,255,255,0.6)'}; background:{loopEnabled ? '#2563eb' : 'rgba(255,255,255,0.07)'}; border:1px solid {loopEnabled ? '#3b82f6' : 'rgba(255,255,255,0.1)'}"
              title="Loop">
        <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round">
          <path d="M17 1l4 4-4 4"/><path d="M3 11V9a4 4 0 0 1 4-4h14"/>
          <path d="M7 23l-4-4 4-4"/><path d="M21 13v2a4 4 0 0 1-4 4H3"/>
        </svg>
      </button>
    </div>

    <!-- Timecodes — right, vertically centred -->
    <div class="absolute right-3 inset-y-0 flex items-center gap-4 font-mono tabular-nums" style="font-size:11px">
      <span>
        <span style="color:rgba(255,255,255,0.5)">FROM START:</span>
        <span style="color:white"> {fmtTC(currentTime)}</span>
      </span>
      <span>
        <span style="color:rgba(255,255,255,0.5)">TO END:</span>
        <span style="color:white"> {fmtTC(Math.max(0, (duration ?? 0) - currentTime))}</span>
      </span>
      <span>
        <span style="color:rgba(255,255,255,0.5)">TOTAL TIME:</span>
        <span style="color:white"> {fmtTC(duration ?? 0)}</span>
      </span>
    </div>

      </div><!-- /controls row -->

      </div><!-- /waveform+controls 176px -->

    </div><!-- /left column -->

    <!-- Right column: VU meter spanning full height (expands with panel) -->
    <div class="relative shrink-0 mr-3 mt-1.5 mb-1" style="width:48px">
      <canvas bind:this={vuCanvas} width="96" height={vizExpanded ? 1040 : 352}
              style="width:48px; height:100%; display:block; border-radius:4px"
      ></canvas>
      <div class="absolute inset-0 pointer-events-none" style="font:8px monospace">
        {#each [[10,'-6'],[20,'-12'],[30,'-18'],[40,'-24'],[80,'-48']] as [pct, lbl]}
          <span class="absolute right-0.5 leading-none"
                style="top:{pct}%; transform:translateY(-50%); color:rgba(255,255,255,0.45)">{lbl}</span>
        {/each}
      </div>
    </div>

  </div><!-- /full-height flex -->

</div>
