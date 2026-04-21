import { describe, it, expect } from 'vitest';
import { createLimiter, defaultBatchConcurrency } from '../lib/concurrency.js';

/** Build a deferred — resolve/reject from outside. */
function deferred() {
  let resolve, reject;
  const promise = new Promise((res, rej) => { resolve = res; reject = rej; });
  return { promise, resolve, reject };
}

describe('createLimiter', () => {
  it('enforces concurrency cap — observed in-flight never exceeds limit', async () => {
    const limit = 3;
    const limiter = createLimiter(limit);
    const total = 20;
    const gates = Array.from({ length: total }, () => deferred());
    let maxObserved = 0;

    const runs = [];
    for (let i = 0; i < total; i++) {
      runs.push(limiter.run(async () => {
        maxObserved = Math.max(maxObserved, limiter.active());
        expect(limiter.active()).toBeLessThanOrEqual(limit);
        await gates[i].promise;
      }));
    }

    // Microtask turn: first `limit` tasks acquire, rest queued.
    await new Promise(r => setTimeout(r, 0));
    expect(limiter.active()).toBe(limit);
    expect(limiter.queued()).toBe(total - limit);

    // Drain in order; each resolve should free exactly one slot.
    for (let i = 0; i < total; i++) gates[i].resolve();
    await Promise.all(runs);

    expect(maxObserved).toBe(limit);
    expect(limiter.active()).toBe(0);
    expect(limiter.queued()).toBe(0);
  });

  it('floors limit at 1 — zero/negative/NaN fall back to serial', async () => {
    for (const bad of [0, -5, NaN, null, undefined]) {
      const limiter = createLimiter(/** @type {any} */(bad));
      const started = [];
      const gates = [deferred(), deferred()];
      const p0 = limiter.run(async () => { started.push(0); await gates[0].promise; });
      const p1 = limiter.run(async () => { started.push(1); await gates[1].promise; });
      await new Promise(r => setTimeout(r, 0));
      expect(started).toEqual([0]);
      gates[0].resolve();
      await new Promise(r => setTimeout(r, 0));
      expect(started).toEqual([0, 1]);
      gates[1].resolve();
      await Promise.all([p0, p1]);
    }
  });

  it('releases the slot on task rejection — pool survives errors', async () => {
    const limiter = createLimiter(2);
    const err = new Error('boom');
    await expect(limiter.run(async () => { throw err; })).rejects.toBe(err);
    await expect(limiter.run(async () => { throw err; })).rejects.toBe(err);
    // Pool is recoverable — a third task runs to completion.
    const result = await limiter.run(async () => 42);
    expect(result).toBe(42);
    expect(limiter.active()).toBe(0);
    expect(limiter.queued()).toBe(0);
  });

  it('processes queued tasks FIFO as slots free', async () => {
    const limiter = createLimiter(1);
    const order = [];
    const gates = [deferred(), deferred(), deferred()];
    const runs = [0, 1, 2].map(i => limiter.run(async () => {
      order.push(i);
      await gates[i].promise;
    }));
    await new Promise(r => setTimeout(r, 0));
    expect(order).toEqual([0]);
    gates[0].resolve();
    await new Promise(r => setTimeout(r, 0));
    expect(order).toEqual([0, 1]);
    gates[1].resolve();
    await new Promise(r => setTimeout(r, 0));
    expect(order).toEqual([0, 1, 2]);
    gates[2].resolve();
    await Promise.all(runs);
  });

  it('returns the task result through run()', async () => {
    const limiter = createLimiter(4);
    const results = await Promise.all([
      limiter.run(async () => 'a'),
      limiter.run(async () => 7),
      limiter.run(async () => ({ ok: true })),
    ]);
    expect(results).toEqual(['a', 7, { ok: true }]);
  });
});

describe('defaultBatchConcurrency', () => {
  it('returns a sane value in the current environment', () => {
    const n = defaultBatchConcurrency();
    expect(n).toBeGreaterThanOrEqual(1);
    expect(n).toBeLessThanOrEqual(8);
    expect(Number.isInteger(n)).toBe(true);
  });

  it('clamps hardwareConcurrency hint into [1, 8]', () => {
    // Stubbing navigator for the duration of the call.
    const origNav = globalThis.navigator;
    try {
      globalThis.navigator = /** @type {any} */ ({ hardwareConcurrency: 32 });
      expect(defaultBatchConcurrency()).toBe(8);
      globalThis.navigator = /** @type {any} */ ({ hardwareConcurrency: 0 });
      expect(defaultBatchConcurrency()).toBe(4); // falls back to 4 when hint is 0
      globalThis.navigator = /** @type {any} */ ({ hardwareConcurrency: 2 });
      expect(defaultBatchConcurrency()).toBe(2);
    } finally {
      globalThis.navigator = origNav;
    }
  });
});
