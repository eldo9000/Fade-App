# TASK-6: Add timeout watchdog to createLimiter

## Goal
`createLimiter` in `src/lib/concurrency.js` gains an optional per-task timeout so that slots cannot be held indefinitely. A task that exceeds its timeout has its slot force-released, and the task's Promise rejects with a timeout error. Existing tests pass; new tests cover the timeout path.

## Context
`createLimiter` is used in `App.svelte` to cap concurrent batch FFmpeg invocations at `hardwareConcurrency`. It works by acquiring a slot before each task and releasing it in a `finally` block after the task's Promise settles.

The slot-leak risk (flagged in audit followups): if a task's Promise never settles — for example, because it awaits a Tauri `job-done` event that is never fired due to a backend crash, a missed event, or app teardown — the slot is held permanently. When all slots are leaked, future batch jobs queue silently and never start. There is no timeout or drain valve.

**Current `createLimiter` shape (`src/lib/concurrency.js`):**
```javascript
export function createLimiter(limit) {
  // ...
  const run = async (task) => {
    await acquire();
    try {
      return await task();
    } finally {
      release();
    }
  };
  return { run, active, queued };
}
```

**Target shape:** Add an optional `timeoutMs` parameter to `run` (NOT to `createLimiter` itself — different tasks may have different timeout tolerances). When provided, `task()` races against a timer; if the timer fires first, the slot is released and the `run` Promise rejects with `new Error('limiter timeout')`. The timer is cleared if the task completes first.

```javascript
const run = async (task, timeoutMs) => {
  await acquire();
  try {
    if (!timeoutMs) return await task();
    return await new Promise((resolve, reject) => {
      const timer = setTimeout(() => reject(new Error('limiter timeout')), timeoutMs);
      task().then(
        (v) => { clearTimeout(timer); resolve(v); },
        (e) => { clearTimeout(timer); reject(e); }
      );
    });
  } finally {
    release();
  }
};
```

The `finally` block always runs, so the slot is released regardless of whether the task completed normally, threw, or timed out.

**Updated return type**: `{ run, active, queued }` — same shape, but `run` now accepts an optional second argument.

**Callers of `createLimiter.run()`** are in `App.svelte`. Check how `run` is called there. If callers currently call `limiter.run(task)` (no second arg), they continue to work unchanged — `timeoutMs` defaults to `undefined` which skips the race.

Optionally, expose a configurable default timeout on the limiter itself (pass `defaultTimeoutMs` to `createLimiter`), so callers that don't pass a per-task timeout still get a safety net. This is a judgment call; if added, it must not break existing tests.

## In scope
- `src/lib/concurrency.js` — `createLimiter` and its `run` function
- `src/tests/concurrency.test.js` — add timeout test cases

## Out of scope
- `App.svelte` callers — do not change call sites unless they need to opt into timeouts; existing calls with no second arg continue to work
- Any Rust/Tauri code
- `defaultBatchConcurrency` function in concurrency.js — do not touch

## Steps
1. Read `src/lib/concurrency.js` in full.
2. Read `src/tests/concurrency.test.js` in full.
3. Update `run` in `createLimiter` to accept an optional `timeoutMs` argument. Implement the race as described in Context.
4. Optionally, add a `defaultTimeoutMs` option to the `createLimiter` factory (passed via options object or second param). If added, `run(task)` with no explicit timeout uses the factory default.
5. Add test cases in `concurrency.test.js`:
   - A task that resolves before its timeout resolves normally and releases the slot.
   - A task that exceeds its timeout rejects with `'limiter timeout'` and releases the slot (so the next task can run).
   - After a timeout, `active()` is decremented back to 0 (slot returned).
6. Run `npm run test` to confirm all tests pass.

## Success signal
`npm run test` passes with all new timeout tests green. `limiter.run(task)` (no timeout) still works identically to before. `limiter.run(task, 5000)` where task hangs indefinitely rejects after 5000ms and releases the slot. After the timeout, `limiter.active()` returns 0.

## Notes
The `finally` block releasing the slot on timeout is the key invariant. Verify the test explicitly checks `active()` after the timeout fires — not just that the Promise rejected.

The existing `concurrency.test.js` uses a gate/resolver pattern (deferred promises that tests resolve manually). Reuse that pattern for the timeout tests.
