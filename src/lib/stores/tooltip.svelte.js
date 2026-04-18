// Shared tooltip/hint store with polished timing:
//   - Fresh show: 100ms fade in
//   - Hover off:  hold 2s, then 2s fade out
//   - Interrupted by new hint: 100ms crossfade (out + in overlap)
//
// Consumer binds to `tooltip.text`, `tooltip.opacity`, `tooltip.duration`,
// `tooltip.delay` and applies them via CSS `opacity` + `transition`.

let _text     = $state('');
let _opacity  = $state(0);
let _duration = $state(100); // ms
let _delay    = $state(0);   // ms

let clearTimer = null;
let swapTimer  = null;

function cancelTimers() {
  if (clearTimer) { clearTimeout(clearTimer); clearTimer = null; }
  if (swapTimer)  { clearTimeout(swapTimer);  swapTimer  = null; }
}

export function setHint(t) {
  const next = t ?? '';
  cancelTimers();

  // Case 1: clearing (hover off). Hold 3s, then fade out over 4s.
  if (!next) {
    _delay    = 3000;
    _duration = 4000;
    _opacity  = 0;
    // Wipe text only after the full hold+fade completes, so mid-fade
    // crossfades still see the old text for the overlap.
    clearTimer = setTimeout(() => { _text = ''; clearTimer = null; }, 7000);
    return;
  }

  // Already showing same text at full opacity? no-op.
  if (_text === next && _opacity === 1 && !clearTimer) return;

  // Case 2: fresh show (no current text or faded out). 100ms fade in.
  if (!_text || _opacity === 0) {
    _text     = next;
    _delay    = 0;
    _duration = 100;
    _opacity  = 1;
    return;
  }

  // Case 3: interrupt current hint with new one. 100ms fade out,
  // then swap & 100ms fade in. Short overlap by design.
  _delay    = 0;
  _duration = 100;
  _opacity  = 0;
  swapTimer = setTimeout(() => {
    _text    = next;
    _opacity = 1;
    swapTimer = null;
  }, 100);
}

export const tooltip = {
  get text()     { return _text; },
  get opacity()  { return _opacity; },
  get duration() { return _duration; },
  get delay()    { return _delay; },
};
