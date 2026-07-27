import type { LogHandler } from '../log/log-handler';
import { LOG_LEVEL_WARN } from '../log/logging';

/**
 * Measure what plugin hooks cost, from inside the JS callback.
 *
 * The driver times hooks on the Rust side, which brackets *dispatch* and *completion*.
 * When a hook is invoked concurrently those two points are separated mostly by the queue
 * the call waited in, so two hooks with very different real costs produce near-identical
 * measurements and cannot be ranked against each other. `HookKind::is_serially_invoked`
 * exists for exactly that reason: serial call sites have no queue, so the driver reports
 * those and stays silent about the rest.
 *
 * Silence is the correct default when the only clock is on the Rust side. It is not the
 * only option, because the missing quantity — when the callback began running — is known
 * here. Starting the clock inside the callback removes the dispatch queue by construction,
 * which is enough to measure many of the hooks the driver has to skip.
 *
 * ## Why `maxInFlight` decides whether a total means anything
 *
 * Entry-to-exit spans may be summed only if they never overlap. A synchronous callback
 * cannot overlap: it holds the thread until it returns. An `async` callback can — it may
 * suspend at an `await` and let another call of the same hook begin, and then both spans
 * cover the same wall clock and the sum counts it twice.
 *
 * Overlap also changes what the span is measuring. A hook that awaits the bundler (say
 * `this.resolve`) spends most of its span waiting for Rust, so the number describes the
 * bundler rather than the plugin. Observed on a real build: one `resolveId` accumulated
 * ~47,000s of span inside a 44s module-loading window.
 *
 * So overlap is counted rather than assumed, per hook:
 *
 * - `maxInFlight === 1` — no call ever overlapped another. The sum **is** execution time.
 * - `maxInFlight > 1` — spans overlap; the total is an upper bound and is not reported as
 *   a cost. The hook is named as needing a sampling profiler instead.
 *
 * This keeps the same rule the Rust side follows — never present a number that cannot be
 * defended — while narrowing "cannot measure" from a property of the *call site* to a
 * property of the *callback*, which is where it actually lives. A hook dispatched
 * concurrently but whose body never overlaps is measured exactly.
 */

/** Nothing is reported below this, matching the driver's own floor for a slow build. */
const MIN_WINDOW_MS = 3_000;
/** A row must reach this to be worth a line. */
const MIN_ROW_MS = 1_000;
const MAX_MEASURED_ROWS = 12;
const MAX_UNMEASURED_ROWS = 8;

export const HOOK_COST = 'HOOK_COST';

interface HookCost {
  pluginName: string;
  hookName: string;
  calls: number;
  ms: number;
  inFlight: number;
  maxInFlight: number;
}

export interface HookCostRecorder {
  costs: Map<string, HookCost>;
  /** First entry and last exit of any measured callback, i.e. the window they ran in. */
  windowStart: number;
  windowEnd: number;
  logHandler: LogHandler;
}

/**
 * One recorder per build, keyed on the object that identifies it.
 *
 * Counters cannot be module-global. A worker sub-build runs *nested inside* the build
 * that spawned it and finishes first, so shared counters let the inner build flush the
 * outer one's half-accumulated totals and report a window belonging to neither.
 */
const recorders = new WeakMap<object, HookCostRecorder>();

/** The recorder for one build, created on first use. */
export function hookCostRecorderFor(key: object, onLog: LogHandler): HookCostRecorder {
  let recorder = recorders.get(key);
  if (recorder === undefined) {
    recorder = { costs: new Map(), windowStart: 0, windowEnd: 0, logHandler: onLog };
    recorders.set(key, recorder);
  }
  return recorder;
}

function costFor(recorder: HookCostRecorder, pluginName: string, hookName: string): HookCost {
  const key = `${pluginName} ${hookName}`;
  let cost = recorder.costs.get(key);
  if (cost === undefined) {
    cost = { pluginName, hookName, calls: 0, ms: 0, inFlight: 0, maxInFlight: 0 };
    recorder.costs.set(key, cost);
  }
  return cost;
}

/**
 * Wrap one hook handler so its execution time is recorded.
 *
 * Returns the handler untouched when there is no recorder, so a build that is not
 * measuring pays nothing beyond one branch at setup.
 */
export function measureHookCost<T extends (...args: never[]) => unknown>(
  recorder: HookCostRecorder | undefined,
  pluginName: string,
  hookName: string,
  handler: T,
): T {
  if (recorder === undefined) {
    return handler;
  }

  const cost = costFor(recorder, pluginName, hookName);

  return function (this: unknown, ...args: never[]) {
    cost.calls += 1;
    cost.inFlight += 1;
    if (cost.inFlight > cost.maxInFlight) {
      cost.maxInFlight = cost.inFlight;
    }
    const started = performance.now();
    if (recorder.windowStart === 0) {
      recorder.windowStart = started;
    }

    const finish = () => {
      recorder.windowEnd = performance.now();
      cost.ms += recorder.windowEnd - started;
      cost.inFlight -= 1;
    };

    let result: unknown;
    try {
      result = handler.apply(this, args);
    } catch (error) {
      finish();
      throw error;
    }

    // Only a thenable defers completion; a plain return value is already done, and
    // treating it as pending would leave `inFlight` permanently raised.
    if (typeof (result as { then?: unknown } | undefined)?.then === 'function') {
      return (result as Promise<unknown>).then(
        (value) => {
          finish();
          return value;
        },
        (error: unknown) => {
          finish();
          throw error;
        },
      );
    }

    finish();
    return result;
  } as T;
}

function formatDuration(ms: number): string {
  return ms >= 1_000 ? `${(ms / 1_000).toFixed(1)}s` : `${Math.round(ms)}ms`;
}

/**
 * Render and emit the report for one build, then drop its recorder. Safe to call when
 * nothing was measured, and when called more than once for the same build.
 *
 * Reports only when the measured window is long enough to be worth a developer's
 * attention, so ordinary fast builds stay quiet.
 */
export function flushHookCostReport(key: object): void {
  const recorder = recorders.get(key);
  if (recorder === undefined) {
    return;
  }
  recorders.delete(key);

  const windowMs = recorder.windowEnd - recorder.windowStart;
  if (windowMs < MIN_WINDOW_MS) {
    return;
  }

  const collected = [...recorder.costs.values()];
  const measured = collected
    .filter((cost) => cost.maxInFlight === 1 && cost.ms >= MIN_ROW_MS)
    .sort((a, b) => b.ms - a.ms);
  // Ordered by how heavily they overlapped: the worst offenders are the ones whose spans
  // are least meaningful, and the most likely to be worth profiling.
  const unmeasured = collected
    .filter((cost) => cost.maxInFlight > 1)
    .sort((a, b) => b.maxInFlight - a.maxInFlight);

  if (measured.length === 0 && unmeasured.length === 0) {
    return;
  }

  const share = (ms: number) => Math.round((ms / windowMs) * 100);
  const lines: string[] = [];

  if (measured.length > 0) {
    const total = measured.reduce((sum, cost) => sum + cost.ms, 0);
    lines.push(
      `Measured inside the plugin callbacks, over a ${formatDuration(windowMs)} window ` +
        `(${share(total)}% attributed):`,
    );
    for (const cost of measured.slice(0, MAX_MEASURED_ROWS)) {
      lines.push(
        `  - ${cost.pluginName} ${cost.hookName} ` +
          `(${share(cost.ms)}%, ${formatDuration(cost.ms)}, ${cost.calls} calls)`,
      );
    }
  }

  if (unmeasured.length > 0) {
    lines.push(
      'Not measurable — these callbacks overlap each other, so their elapsed time covers',
      'work other calls were doing. A hook awaiting the bundler measures the bundler, not',
      'itself. Profile them with `node --cpu-prof`:',
    );
    for (const cost of unmeasured.slice(0, MAX_UNMEASURED_ROWS)) {
      lines.push(`  - ${cost.pluginName} ${cost.hookName} (${cost.calls} calls)`);
    }
  }

  recorder.logHandler(LOG_LEVEL_WARN, {
    code: HOOK_COST,
    message: lines.join('\n'),
  });
}
