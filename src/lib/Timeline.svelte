<script>
  import { convertFileSrc, invoke } from '@tauri-apps/api/core';
  import { listen } from '@tauri-apps/api/event';
  import { setHint } from './stores/tooltip.svelte.js';

  let { item, duration = null, options = $bindable(null), mediaEl = null, onscrubstart = null, vizExpanded = $bindable(false), mediaReady = false, waveformReady = false, spectrogramReady = false, filmstripReady = false, cachedWaveform = null, cachedFilmstripFrames = null, draft = false, replacedAudioMode = false } = $props();

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
  let _gainNode     = null; // GainNode for volume control on owned-internal (audio-only) path

  function _teardownGraph() {
    if (_rafId) { cancelAnimationFrame(_rafId); _rafId = null; }
    if (_shadowEl) { _shadowEl.pause(); _shadowEl.src = ''; _shadowEl = null; }
    if (_audioCtx) { _audioCtx.close().catch(() => {}); _audioCtx = null; }
    _analyserL = _analyserR = _splitter = _gainNode = null;
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
  //
  // Strategy depends on who owns the media element:
  //
  //  _ownedInternal = true  (audio-only files — we created a fresh Audio() per file)
  //    → createMediaElementSource(audioEl) directly. Each file is a new element so
  //      "already connected" never applies. Audio is routed through Web Audio;
  //      connect splitter output to ctx.destination so the user still hears it.
  //
  //  _ownedInternal = false  (video files — parent's <video> element is reused)
  //    → captureStream() if available (no routing change, zero latency).
  //      Otherwise a silent shadow Audio element routed through Web Audio —
  //      analysers only, main video element keeps its native audio path.
  //      Shadow is preloaded before play() so it's ready when play() fires.
  function _connectSource() {
    if (_srcConnected || !audioEl || !_audioCtx || !_splitter) return;
    try {
      if (_ownedInternal) {
        // Audio file: route the owned element through Web Audio.
        // audioEl is a fresh new Audio() per file — no "already connected" risk.
        // GainNode sits between source and destination so _applyEnvelope can control
        // volume reliably — audioEl.volume has no effect once createMediaElementSource
        // takes over (WKWebView does not forward it through the Web Audio graph).
        const src = _audioCtx.createMediaElementSource(audioEl);
        const gain = _audioCtx.createGain();
        src.connect(_splitter);
        src.connect(gain);
        gain.connect(_audioCtx.destination);
        _gainNode = gain;
        _srcConnected = true;
      } else if (typeof audioEl.captureStream === 'function') {
        // Video file — best path: tap a copy of the native output stream.
        const src = _audioCtx.createMediaStreamSource(audioEl.captureStream());
        src.connect(_splitter);
        _srcConnected = true;
      } else {
        // Video file — fallback: silent shadow Audio element.
        // The main <video> keeps its native audio path (no latency added).
        // Preload so the element is ready when play() calls shadow.play().
        const shadow = new Audio(audioEl.src);
        shadow.preload = 'auto';
        shadow.volume  = 0;
        shadow.currentTime = audioEl.currentTime;
        shadow.loop    = audioEl.loop;
        shadow.load();   // start buffering now, inside user-gesture if called from play()
        const src = _audioCtx.createMediaElementSource(shadow);
        src.connect(_splitter);
        // GainNode keeps shadow silent (createMediaElementSource routes to destination).
        const mute = _audioCtx.createGain();
        mute.gain.value = 0;
        src.connect(mute);
        mute.connect(_audioCtx.destination);
        _shadowEl = shadow;
        _srcConnected = true;
      }
    } catch (e) { console.error('[viz] connectSource failed:', e); }
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
    const gn = _gainNode;
    const from = gn ? gn.gain.value : el.volume;
    const id = ++_declickId;
    _declickActive = true;
    const start = performance.now();
    const tick = () => {
      if (_declickId !== id || audioEl !== el) { _declickActive = false; return; }
      const t = Math.min(1, (performance.now() - start) / durationMs);
      const val = from + (to - from) * t;
      if (gn) gn.gain.value = val; else el.volume = val;
      if (t < 1) setTimeout(tick, 0);
      else { _declickActive = false; onDone?.(); }
    };
    setTimeout(tick, 0);
  }

  // Fade curve with guaranteed zero slope at BOTH endpoints for all tension values.
  // Uses mirrored smoothstep composition — proven smooth at 0 and 1 for all exp > 0.
  // Positive tension → convex S (fast-then-slow). Negative → concave S (slow-then-fast).
  function _fadeCurve(p, tension) {
    const pc = Math.max(0, Math.min(1, p));
    if (tension >= 0) {
      // pow(smoothstep(p), exp): slope at p=0 → smoothstep'(0)=0 kills product. ✓
      //                          slope at p=1 → smoothstep'(1)=0 kills product. ✓
      const s = pc * pc * (3 - 2 * pc);
      return Math.pow(s, Math.pow(2, tension));
    } else {
      // Mirror: 1 − pow(smoothstep(1−p), exp) — symmetric, same guarantee for all exp. ✓
      const q = 1 - pc;
      const s = q * q * (3 - 2 * q);
      return 1 - Math.pow(s, Math.pow(2, -tension));
    }
  }

  // Compute the fade-envelope amplitude [0,1] for a given playback time.
  function _computeFadeEnvelope(t) {
    const trimStart = options?.trim_start ?? 0;
    const trimEnd   = options?.trim_end   ?? duration ?? 0;
    const fadeIn    = options?.fade_in    ?? 0;
    const fadeOut   = options?.fade_out   ?? 0;
    let vol = 1;
    if (fadeIn > 0 && t < trimStart + fadeIn) {
      const p = Math.max(0, (t - trimStart) / fadeIn);
      vol = Math.min(vol, _fadeCurve(p, options?.fade_in_curve ?? 0));
    }
    if (fadeOut > 0 && t > trimEnd - fadeOut) {
      const p = Math.max(0, (trimEnd - t) / fadeOut);
      vol = Math.min(vol, _fadeCurve(p, options?.fade_out_curve ?? 0));
    }
    return vol;
  }

  // Apply fade envelope to audio volume and video brightness.
  // Called from the RAF loop (while playing) and on timeupdate (while scrubbing).
  function _applyEnvelope() {
    const t = audioEl?.currentTime ?? currentTime;
    const vol = _computeFadeEnvelope(t);
    // Audio: only override when de-click is not active (de-click wins briefly on play/pause).
    // Use GainNode when available — audioEl.volume has no effect once the element is routed
    // through createMediaElementSource on WKWebView.
    if (!_declickActive && isPlaying) {
      if (_gainNode) _gainNode.gain.value = vol;
      else if (audioEl) audioEl.volume = vol;
    }
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
    if (_gainNode) _gainNode.gain.value = 0; else audioEl.volume = 0;
    audioEl.play().catch(() => { isPlaying = false; });
    if (_shadowEl) { _shadowEl.currentTime = audioEl.currentTime; _shadowEl.play().catch(() => {}); }
    _declick(1, 25, null);
  }

  function pause() {
    if (!audioEl) return;
    isPlaying = false;
    const el = audioEl;
    const sh = _shadowEl;
    const gn = _gainNode;
    _declick(0, 25, () => { el.pause(); if (gn) gn.gain.value = 1; else el.volume = 1; sh?.pause(); if (mediaEl) mediaEl.style.filter = ''; });
  }

  function stop() {
    if (!audioEl) return;
    isPlaying = false;
    const el = audioEl;
    const sh = _shadowEl;
    const gn = _gainNode;
    _declick(0, 25, () => { el.pause(); if (gn) gn.gain.value = 1; else el.volume = 1; sh?.pause(); seekTo(options?.trim_start ?? 0); if (mediaEl) mediaEl.style.filter = ''; });
  }

  // Seek with de-click: always mute → seek → fade back in so the initial click
  // is inaudible even when audioEl.play() (called by _beginScrub) hasn't resolved
  // yet (play() is async, so audioEl.paused may still be true at call time).
  function seekWithDeclick(secs) {
    if (!audioEl) return;
    if (_gainNode) _gainNode.gain.value = 0; else audioEl.volume = 0;
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
    if (!cv) return;
    const ctx = cv.getContext('2d');
    const w = cv.width, h = cv.height, cx = w / 2, cy = h / 2;

    if (!_analyserL || !_analyserR) {
      // No analyser — draw static background + crosshairs
      ctx.fillStyle = '#000';
      ctx.fillRect(0, 0, w, h);
      ctx.strokeStyle = 'rgba(255,255,255,0.07)';
      ctx.lineWidth = 1;
      ctx.beginPath();
      ctx.moveTo(cx, 0); ctx.lineTo(cx, h);
      ctx.moveTo(0, cy); ctx.lineTo(w, cy);
      ctx.stroke();
      return;
    }

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
    if (!cv) return;
    const ctx = cv.getContext('2d');
    const w = cv.width, h = cv.height;

    if (!_analyserL || !_analyserR) {
      // No analyser — draw static grid only
      ctx.fillStyle = '#080808';
      ctx.fillRect(0, 0, w, h);
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
      return;
    }

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
    if (!cv) return;
    const ctx = cv.getContext('2d');
    const w = cv.width, h = cv.height;

    if (!_analyserL || !_analyserR) {
      // No analyser — draw static grid only
      ctx.fillStyle = '#080808';
      ctx.fillRect(0, 0, w, h);
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
      return;
    }

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
    if (!cv) return;
    const ctx = cv.getContext('2d');
    const w = cv.width, h = cv.height;

    ctx.fillStyle = '#080808';
    ctx.fillRect(0, 0, w, h);

    if (!_analyserL || !_analyserR) return;

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
    // Also track mediaReady: when it transitions false→true the media $effect calls
    // _teardownGraph() which kills _rafId, then _initCtx(). Without tracking
    // mediaReady here the RAF never restarts (vizExpanded/isPlaying didn't change).
    void mediaReady;
    if (isPlaying || vizExpanded) {
      if (!_rafId) _rafId = requestAnimationFrame(_renderLoop);
    } else {
      if (_rafId) { cancelAnimationFrame(_rafId); _rafId = null; }
    }
    return () => { if (_rafId) { cancelAnimationFrame(_rafId); _rafId = null; } };
  });

  // Connect audio source when viz panel opens before playback has started
  $effect(() => {
    if (vizExpanded && mediaReady && audioEl && _audioCtx && !_srcConnected) {
      _connectSource();
    }
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
    const isDraft = draft;
    waveformData = null;
    if (!it || !go) return;
    const id = it.id; _capturedId = id;
    mediaLoading = true;
    invoke('get_waveform', { path: it.path, draft: isDraft, buckets: 4000 })
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
    const cached = cachedFilmstripFrames;
    const isDraft = draft;
    filmstripFrames = [];
    if (!it || !go || !dur || it.mediaType !== 'video') return;

    // If we have pre-loaded frames (full or partial), use them immediately.
    // For partial (bg preload still in-flight), listen for the remaining -bg events
    // instead of firing a new ffmpeg call — no double-loading.
    if (cached && cached.length > 0) {
      filmstripFrames = [...cached];
      if (cached.some(f => f === null)) {
        const bgId = it.id + '-bg';
        let unlisten = null;
        listen('filmstrip-frame', (ev) => {
          const { id, index, data } = ev.payload;
          if (id !== bgId) return;
          filmstripFrames[index] = data;
        }).then(fn => { unlisten = fn; });
        return () => { unlisten?.(); };
      }
      return; // Fully cached — instant, no cleanup needed
    }

    const myId = it.id;
    // Keep 20 frames in both modes — scale reduction alone handles throughput.
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
    invoke('get_filmstrip', { path: it.path, id: myId, count: COUNT, duration: dur, draft: isDraft })
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
      ? options.trim_start / duration : 0);
  let endFrac = $derived(
    duration && options?.trim_end != null
      ? options.trim_end / duration : 1);
  let playFrac = $derived(duration ? Math.max(0, Math.min(1, currentTime / duration)) : 0);

  // ── Silence padding fractions (relative to total displayed width) ─────────
  // Silence padding only applies to audio output; suppress for video.
  let padFrontSecs   = $derived(item?.mediaType === 'video' ? 0 : (options?.pad_front ?? 0));
  let padEndSecs     = $derived(item?.mediaType === 'video' ? 0 : (options?.pad_end   ?? 0));
  let totalDispSecs  = $derived((duration ?? 0) + padFrontSecs + padEndSecs);
  let silFrontFrac   = $derived(totalDispSecs > 0 ? padFrontSecs / totalDispSecs : 0);
  let silEndFrac     = $derived(totalDispSecs > 0 ? padEndSecs   / totalDispSecs : 0);
  let audioWidthFrac = $derived(Math.max(0, 1 - silFrontFrac - silEndFrac));

  // Fade handle fractions — clamped so they stay inside [startFrac, endFrac].
  let fadeInFrac = $derived(
    duration ? Math.min(endFrac, Math.max(startFrac,
      ((options?.trim_start ?? 0) + (options?.fade_in ?? 0)) / duration)) : startFrac);
  let fadeOutFrac = $derived(
    duration ? Math.max(startFrac, Math.min(endFrac,
      ((options?.trim_end ?? duration ?? 0) - (options?.fade_out ?? 0)) / duration)) : endFrac);

  // Curve polyline points for SVG ramp lines — matches _fadeCurve exactly (40 segments)
  let fadeInCurvePoints = $derived.by(() => {
    const tension = options?.fade_in_curve ?? 0;
    const pts = [];
    for (let i = 0; i <= 40; i++) {
      const p = i / 40;
      pts.push(`${p * 100},${(1 - _fadeCurve(p, tension)) * 100}`);
    }
    return pts.join(' ');
  });
  let fadeOutCurvePoints = $derived.by(() => {
    const tension = options?.fade_out_curve ?? 0;
    const pts = [];
    for (let i = 0; i <= 40; i++) {
      const p = i / 40; // 0=handle, 1=trim end
      pts.push(`${p * 100},${(1 - _fadeCurve(1 - p, tension)) * 100}`);
    }
    return pts.join(' ');
  });

  // ── Drag ──────────────────────────────────────────────────────────────────
  let trackEl       = $state(null);
  let filmstripEl   = $state(null);
  let dragging      = $state(null); // 'start' | 'end' | 'playhead' | 'fade_in' | 'fade_out'
  let _dragEl       = null; // element the current drag started on
  let _wasPlayingBeforeDrag = false;
  let _dragStartY     = 0;   // Y position at fade-handle mousedown
  let _dragStartCurve = 0;   // curve value at fade-handle mousedown

  // ── Zoom / pan ────────────────────────────────────────────────────────────
  let viewportEl = $state(null);
  let zoom       = $state(1);
  let panCenter  = $state(0.5);
  let _panning   = $state(false);
  let _panStartX = 0;
  let _panStartPan = 0.5;
  let widthPct  = $derived(zoom * 100);
  let leftPct   = $derived((0.5 - panCenter * zoom) * 100);
  // Bars get visually thin at low zoom; boost opacity there. 2× at zoom=1 → baseline at zoom≥4.
  let waveOpacity = $derived(0.525 * (2 - Math.min(1, (zoom - 1) / 3)));

  function _clampPan(p, z) {
    if (z <= 1) return 0.5;
    const half = 0.5 / z;
    return Math.max(half, Math.min(1 - half, p));
  }
  function onTrackWheel(e) {
    if (!viewportEl) return;
    e.preventDefault();
    const r = viewportEl.getBoundingClientRect();
    const v = (e.clientX - r.left) / r.width;
    const factor = e.deltaY < 0 ? 1.32 : 1 / 1.32;
    const newZoom = Math.max(1, Math.min(20, zoom * factor));
    if (newZoom === zoom) return;
    const contentFrac = panCenter + (v - 0.5) / zoom;
    panCenter = _clampPan(contentFrac - (v - 0.5) / newZoom, newZoom);
    zoom = newZoom;
  }
  function onTrackAuxDown(e) {
    if (e.button !== 1 && e.button !== 2) return;
    e.preventDefault();
    e.stopPropagation();
    if (zoom <= 1) return;
    _panning = true;
    _panStartX = e.clientX;
    _panStartPan = panCenter;
  }

  function getFrac(e) {
    const el = _dragEl ?? trackEl;
    if (!el) return 0;
    const r = el.getBoundingClientRect();
    return (e.clientX - r.left) / r.width; // unclamped; trim may extend into silence padding
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
      const gn = _gainNode;
      _declick(0, 25, () => { audioEl?.pause(); if (gn) gn.gain.value = 1; else if (audioEl) audioEl.volume = 1; sh?.pause(); });
    }
    _wasPlayingBeforeDrag = false;
  }

  function onTrackDown(e) {
    if (e.button !== 0) return; // only left click seeks; middle/right bubble to viewport for pan
    if (!duration) return;
    _dragEl = trackEl;
    onscrubstart?.();
    dragging = 'playhead';
    seekWithDeclick(fracToSecs(getFrac(e)));
  }
  function onFilmstripDown(e) {
    if (e.button !== 0) return;
    if (!duration) return;
    _dragEl = filmstripEl;
    onscrubstart?.();
    dragging = 'playhead';
    seekWithDeclick(fracToSecs(getFrac(e)));
  }
  function onWindowMouseMove(e) {
    if (_panning && viewportEl) {
      const r = viewportEl.getBoundingClientRect();
      const dx = (e.clientX - _panStartX) / r.width;
      panCenter = _clampPan(_panStartPan - dx / zoom, zoom);
      return;
    }
    if (!dragging || !duration) return;
    const f = getFrac(e);
    // Clamp trim to actual audio range — silence padding is not croppable.
    if (dragging === 'start') {
      // Block against trim_end and against the fade-in handle hitting fade-out.
      const maxF = Math.min(endFrac, fadeOutFrac - (options?.fade_in ?? 0) / duration);
      options.trim_start = fracToSecs(Math.max(0, Math.min(f, maxF)));
    }
    else if (dragging === 'end') {
      const minF = Math.max(startFrac, fadeInFrac + (options?.fade_out ?? 0) / duration);
      options.trim_end = fracToSecs(Math.min(1, Math.max(f, minF)));
    }
    else if (dragging === 'fade_in') {
      // Horizontal: block against fade-out handle.
      options.fade_in = Math.max(0, fracToSecs(Math.min(f, fadeOutFrac)) - (options.trim_start ?? 0));
      const dy = e.clientY - _dragStartY;
      options.fade_in_curve = Math.max(-3, Math.min(3, _dragStartCurve + dy / 100));
    }
    else if (dragging === 'fade_out') {
      options.fade_out = Math.max(0, (options.trim_end ?? duration ?? 0) - fracToSecs(Math.max(f, fadeInFrac)));
      const dy = e.clientY - _dragStartY;
      options.fade_out_curve = Math.max(-3, Math.min(3, _dragStartCurve + dy / 100));
    }
    else seekTo(fracToSecs(Math.max(0, Math.min(1, f))));
  }
  function onWindowMouseUp() {
    if (dragging === 'playhead') _endScrub();
    dragging = null;
    _dragEl = null;
    _panning = false;
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
        <div class="shrink-0 px-3 pt-2" style="height:224px">
          {#if spectrogramData}
            <img src="data:image/png;base64,{spectrogramData}" alt="Spectrogram"
                 class="w-full h-full object-fill rounded" style="display:block" />
          {:else}
            <div class="w-full h-full rounded" style="background:#111"></div>
          {/if}
        </div>

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
            </div>
          {/if}
        </div>
      {/if}

      <!-- Waveform track + controls (fixed 176px) -->
      <div class="shrink-0 flex flex-col relative" style="height:176px">

      <!-- Viewport (clips zoomed content; wheel + middle/right-click pan) -->
      <!-- svelte-ignore a11y_no_static_element_interactions -->
      <div bind:this={viewportEl}
           class="flex-1 min-h-0 relative mx-3 mt-1.5 mb-1 overflow-hidden"
           style="cursor:{_panning ? 'grabbing' : 'auto'}"
           onwheel={onTrackWheel}
           onmousedown={onTrackAuxDown}
           oncontextmenu={(e) => { if (zoom > 1) e.preventDefault(); }}>
      <!-- Zoom/pan transform layer -->
      <div class="absolute inset-y-0" style="left:{leftPct}%; width:{widthPct}%; transition:left 0.18s ease, width 0.18s ease">
      <!-- Silence region: front (audio only) -->
      {#if silFrontFrac > 0 && item?.mediaType !== 'video'}
        <div class="absolute inset-y-0 left-0 rounded-l overflow-hidden pointer-events-none"
             style="width:{silFrontFrac * 100}%; background:#0e0e0e; border-right:1px dashed rgba(96,165,250,0.25); transition:width 0.18s ease">
          <svg width="100%" height="100%" preserveAspectRatio="none" viewBox="0 0 10 100" xmlns="http://www.w3.org/2000/svg">
            <line x1="0" y1="50" x2="10" y2="50" stroke="rgba(96,165,250,0.45)" stroke-width="1" vector-effect="non-scaling-stroke" />
          </svg>
        </div>
      {/if}
      <!-- Silence region: end (audio only) -->
      {#if silEndFrac > 0 && item?.mediaType !== 'video'}
        <div class="absolute inset-y-0 right-0 rounded-r overflow-hidden pointer-events-none"
             style="width:{silEndFrac * 100}%; background:#0e0e0e; border-left:1px dashed rgba(96,165,250,0.25); transition:width 0.18s ease">
          <svg width="100%" height="100%" preserveAspectRatio="none" viewBox="0 0 10 100" xmlns="http://www.w3.org/2000/svg">
            <line x1="0" y1="50" x2="10" y2="50" stroke="rgba(96,165,250,0.45)" stroke-width="1" vector-effect="non-scaling-stroke" />
          </svg>
        </div>
      {/if}
      <!-- Track (waveform + trim + playhead) -->
      <!-- svelte-ignore a11y_no_static_element_interactions -->
      <div bind:this={trackEl}
           class="absolute inset-y-0 rounded cursor-crosshair"
           style="left:{silFrontFrac * 100}%; width:{audioWidthFrac * 100}%; transition:left 0.18s ease, width 0.18s ease"
           onmousedown={onTrackDown}>

    <!-- Background / waveform -->
    <div class="absolute inset-0 rounded overflow-hidden" style="background:#161616">
      {#if waveformData && waveformData.amplitudes.length > 0}
        <svg class="w-full h-full" preserveAspectRatio="none"
             viewBox="0 0 {waveformData.amplitudes.length} 100" xmlns="http://www.w3.org/2000/svg">
          <g style="opacity:{waveOpacity}; transition:opacity 0.18s ease">
            {#each waveformData.amplitudes as amp, i}
              {@const h = Math.max(1, amp * 96)}
              {@const y = (100 - h) / 2}
              <rect x={i} y={y} width={0.85} height={h}
                    fill={replacedAudioMode
                      ? `hsl(${150 + (waveformData.hues[i] % 80)},75%,55%)`
                      : `hsl(${waveformData.hues[i]},100%,55%)`} />
            {/each}
          </g>
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
           style="width:{startFrac * 100}%; background:rgba(0,0,0,0.85)"></div>
      <!-- Active region -->
      <div class="absolute inset-y-0 pointer-events-none"
           style="left:{startFrac * 100}%; width:{(endFrac - startFrac) * 100}%;
                  border-top:1px solid rgba(255,255,255,0.08);
                  border-bottom:1px solid rgba(255,255,255,0.08);
                  background:rgba(255,255,255,0.03)"></div>
      <!-- Post-trim dim -->
      <div class="absolute inset-y-0 right-0 rounded-r pointer-events-none"
           style="left:{endFrac * 100}%; background:rgba(0,0,0,0.85)"></div>
      <!-- Fade-in gradient overlay — opaque at trim start, transparent at fade handle -->
      {#if (options?.fade_in ?? 0) > 0}
        <div class="absolute inset-y-0 pointer-events-none"
             style="left:{startFrac * 100}%; width:{(fadeInFrac - startFrac) * 100}%;
                    background:linear-gradient(to right, rgba(0,0,0,0.72), transparent)">
        </div>
      {/if}
      <!-- Fade-out gradient overlay — transparent at fade handle, opaque at trim end -->
      {#if (options?.fade_out ?? 0) > 0}
        <div class="absolute inset-y-0 pointer-events-none"
             style="left:{fadeOutFrac * 100}%; width:{(endFrac - fadeOutFrac) * 100}%;
                    background:linear-gradient(to right, transparent, rgba(0,0,0,0.72))">
        </div>
      {/if}
      <!-- Fade-in ramp curve — from trim start (bottom) to fade handle (top) -->
      {#if (options?.fade_in ?? 0) > 0}
        <div class="absolute inset-y-0 pointer-events-none"
             style="left:{startFrac * 100}%; width:{(fadeInFrac - startFrac) * 100}%">
          <svg width="100%" height="100%" preserveAspectRatio="none" viewBox="0 0 100 100" xmlns="http://www.w3.org/2000/svg">
            <polyline points={fadeInCurvePoints} fill="none" stroke="rgba(255,255,255,0.35)" stroke-width="2" vector-effect="non-scaling-stroke"/>
          </svg>
        </div>
      {/if}
      <!-- Fade-out ramp curve — from fade handle (top) to trim end (bottom) -->
      {#if (options?.fade_out ?? 0) > 0}
        <div class="absolute inset-y-0 pointer-events-none"
             style="left:{fadeOutFrac * 100}%; width:{(endFrac - fadeOutFrac) * 100}%">
          <svg width="100%" height="100%" preserveAspectRatio="none" viewBox="0 0 100 100" xmlns="http://www.w3.org/2000/svg">
            <polyline points={fadeOutCurvePoints} fill="none" stroke="rgba(255,255,255,0.35)" stroke-width="2" vector-effect="non-scaling-stroke"/>
          </svg>
        </div>
      {/if}
      <!-- Fade-in triangle handle (sits at fadeInFrac, top of track) -->
      <!-- svelte-ignore a11y_no_static_element_interactions -->
      <div class="absolute z-25 cursor-move"
           style="left:{fadeInFrac * 100}%; top:0; transform:translateX(-50%)"
           onmouseenter={() => { fadeInHovered = true; if ((options?.fade_in ?? 0) > 0) setHint('← drag → length  ·  ↕ curve  ·  right-click reset'); }}
           onmouseleave={() => { fadeInHovered = false; setHint(''); }}
           onmousedown={(e) => { e.stopPropagation(); _dragStartY = e.clientY; _dragStartCurve = options?.fade_in_curve ?? 0; dragging = 'fade_in'; }}
           oncontextmenu={(e) => { e.preventDefault(); e.stopPropagation(); if (options) options.fade_in_curve = 0; }}>
        <svg width="14" height="10" viewBox="0 0 14 10" xmlns="http://www.w3.org/2000/svg">
          <polygon points="7,10 0,0 14,0"
                   fill="{(dragging==='fade_in'||fadeInHovered) ? 'rgba(74,222,128,1)' : 'rgba(74,222,128,0.65)'}"/>
        </svg>
      </div>
      <!-- Fade-out triangle handle (sits at fadeOutFrac, top of track) -->
      <!-- svelte-ignore a11y_no_static_element_interactions -->
      <div class="absolute z-25 cursor-move"
           style="left:{fadeOutFrac * 100}%; top:0; transform:translateX(-50%)"
           onmouseenter={() => { fadeOutHovered = true; if ((options?.fade_out ?? 0) > 0) setHint('← drag → length  ·  ↕ curve  ·  right-click reset'); }}
           onmouseleave={() => { fadeOutHovered = false; setHint(''); }}
           onmousedown={(e) => { e.stopPropagation(); _dragStartY = e.clientY; _dragStartCurve = options?.fade_out_curve ?? 0; dragging = 'fade_out'; }}
           oncontextmenu={(e) => { e.preventDefault(); e.stopPropagation(); if (options) options.fade_out_curve = 0; }}>
        <svg width="14" height="10" viewBox="0 0 14 10" xmlns="http://www.w3.org/2000/svg">
          <polygon points="7,10 0,0 14,0"
                   fill="{(dragging==='fade_out'||fadeOutHovered) ? 'rgba(74,222,128,1)' : 'rgba(74,222,128,0.65)'}"/>
        </svg>
      </div>
      <!-- Trim handles moved to an unclipped overlay sibling of the viewport
           (below) so the nugget icons poke outside the viewport's
           overflow-hidden bounds at far edges. -->
    {/if}

    <!-- Playhead -->
    {#if duration}
      <!-- svelte-ignore a11y_no_static_element_interactions -->
      <div class="absolute inset-y-0 z-30 cursor-ew-resize"
           style="left:{playFrac * 100}%; transform:translateX(-50%)"
           onmousedown={(e) => { if (e.button !== 0) return; e.stopPropagation(); dragging = 'playhead'; }}>
        <div class="absolute top-0 bottom-0 left-1/2 -translate-x-px w-px"
             style="background:#60a5fa"></div>
      </div>
    {/if}
      </div><!-- /track -->
      </div><!-- /transform layer -->
      </div><!-- /viewport -->

      <!-- ── Unclipped handles overlay ──────────────────────────────────
           Lives outside viewport's overflow-hidden, so nugget icons can
           poke into the padding at left/right edges. Mirrors viewport's
           mx-3 and vertical bounds so handle x-coords match the waveform
           underneath. Transform + track position wrappers duplicated so
           zoom/pan/silence math matches the viewport exactly. -->
      {#if showTrim}
        <div class="absolute pointer-events-none" style="left:12px; right:12px; top:6px; bottom:4px; z-index:50">
          <div class="absolute inset-y-0 overflow-visible" style="left:{leftPct}%; width:{widthPct}%; transition:left 0.18s ease, width 0.18s ease">
            <div class="absolute inset-y-0 overflow-visible" style="left:{silFrontFrac * 100}%; width:{audioWidthFrac * 100}%; transition:left 0.18s ease, width 0.18s ease">
              <!-- Left handle -->
              <div class="absolute inset-y-0 pointer-events-none flex items-center justify-center"
                   style="left:calc({startFrac * 100}% - 7px); width:14px">
                <div class="absolute inset-y-0 w-[2px] rounded-full"
                     style="background:{(dragging==='start'||startHovered) ? 'rgba(255,255,255,1)' : 'rgba(255,255,255,0.38)'}; transition:background 0.12s"></div>
                <!-- svelte-ignore a11y_no_static_element_interactions -->
                <div class="relative z-10 pointer-events-auto cursor-ew-resize flex items-center gap-px px-[3px] py-[5px] rounded-sm"
                     onmouseenter={() => startHovered = true}
                     onmouseleave={() => startHovered = false}
                     onmousedown={(e) => { e.stopPropagation(); dragging = 'start'; }}
                     style="background:{(dragging==='start'||startHovered) ? 'rgba(255,255,255,0.34)' : 'rgba(255,255,255,0.12)'}; border:1px solid {(dragging==='start'||startHovered) ? 'rgba(255,255,255,0.72)' : 'rgba(255,255,255,0.22)'}; box-shadow:0 0 0 1px rgba(0,0,0,0.55), 0 1px 3px rgba(0,0,0,0.5); transition:background 0.12s, border-color 0.12s">
                  <svg width="3" height="7" viewBox="0 0 3 7" style="fill:{(dragging==='start'||startHovered) ? 'rgba(255,255,255,1)' : 'rgba(255,255,255,0.55)'}; transition:fill 0.12s"><path d="M3 0 L0 3.5 L3 7 Z"/></svg>
                  <svg width="3" height="7" viewBox="0 0 3 7" style="fill:{(dragging==='start'||startHovered) ? 'rgba(255,255,255,1)' : 'rgba(255,255,255,0.55)'}; transition:fill 0.12s"><path d="M0 0 L3 3.5 L0 7 Z"/></svg>
                </div>
              </div>
              <!-- Right handle -->
              <div class="absolute inset-y-0 pointer-events-none flex items-center justify-center"
                   style="left:calc({endFrac * 100}% - 7px); width:14px">
                <div class="absolute inset-y-0 w-[2px] rounded-full"
                     style="background:{(dragging==='end'||endHovered) ? 'rgba(255,255,255,1)' : 'rgba(255,255,255,0.38)'}; transition:background 0.12s"></div>
                <!-- svelte-ignore a11y_no_static_element_interactions -->
                <div class="relative z-10 pointer-events-auto cursor-ew-resize flex items-center gap-px px-[3px] py-[5px] rounded-sm"
                     onmouseenter={() => endHovered = true}
                     onmouseleave={() => endHovered = false}
                     onmousedown={(e) => { e.stopPropagation(); dragging = 'end'; }}
                     style="background:{(dragging==='end'||endHovered) ? 'rgba(255,255,255,0.34)' : 'rgba(255,255,255,0.12)'}; border:1px solid {(dragging==='end'||endHovered) ? 'rgba(255,255,255,0.72)' : 'rgba(255,255,255,0.22)'}; box-shadow:0 0 0 1px rgba(0,0,0,0.55), 0 1px 3px rgba(0,0,0,0.5); transition:background 0.12s, border-color 0.12s">
                  <svg width="3" height="7" viewBox="0 0 3 7" style="fill:{(dragging==='end'||endHovered) ? 'rgba(255,255,255,1)' : 'rgba(255,255,255,0.55)'}; transition:fill 0.12s"><path d="M3 0 L0 3.5 L3 7 Z"/></svg>
                  <svg width="3" height="7" viewBox="0 0 3 7" style="fill:{(dragging==='end'||endHovered) ? 'rgba(255,255,255,1)' : 'rgba(255,255,255,0.55)'}; transition:fill 0.12s"><path d="M0 0 L3 3.5 L0 7 Z"/></svg>
                </div>
              </div>
            </div>
          </div>
        </div>
      {/if}

      <!-- Controls row -->
      <div class="relative shrink-0" style="height:34px">

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

    <!-- Fade handle hint moved to global hint box (right sidebar footer). -->
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
    <div class="relative shrink-0 mr-3 mt-1.5 mb-[16px]" style="width:48px">
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
