/**
 * Tiny concurrency limiter. Wraps a pool of N slots; callers enqueue async
 * tasks and get back a promise that resolves when the task does. No deps.
 *
 * @param {number} limit  Max concurrent in-flight tasks. Floored at 1.
 * @returns {{
 *   run: <T>(task: () => Promise<T>) => Promise<T>,
 *   active: () => number,
 *   queued: () => number,
 * }}
 */
export function createLimiter(limit) {
  const cap = Math.max(1, Math.floor(limit) || 1);
  let inFlight = 0;
  /** @type {Array<() => void>} */
  const waiters = [];

  const acquire = () => {
    if (inFlight < cap) {
      inFlight++;
      return Promise.resolve();
    }
    return new Promise(resolve => { waiters.push(resolve); });
  };

  const release = () => {
    const next = waiters.shift();
    if (next) next();
    else inFlight--;
  };

  const run = async (task) => {
    await acquire();
    try {
      return await task();
    } finally {
      release();
    }
  };

  return {
    run,
    active: () => inFlight,
    queued: () => waiters.length,
  };
}

/**
 * Default concurrency for batch ffmpeg invokes. Uses the browser's
 * hardwareConcurrency hint if present; falls back to 4. Clamped to [1, 8]
 * because ffmpeg is already multithreaded — more than ~8 concurrent
 * encodes thrashes the CPU regardless of core count.
 */
export function defaultBatchConcurrency() {
  const n = (typeof navigator !== 'undefined' && navigator.hardwareConcurrency) || 4;
  return Math.min(8, Math.max(1, n));
}
