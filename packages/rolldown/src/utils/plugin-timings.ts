import type { LogHandler } from '../log/log-handler';
import { LOG_LEVEL_WARN } from '../log/logging';

/**
 * Measure what plugin hooks cost, from inside the JavaScript callback.
 *
 * The bundler core can only bracket *dispatch* and *completion*. For a hook it invokes
 * concurrently those two points are separated mostly by the queue the call waited in on
 * JavaScript's single thread, and the queue is deepest behind whichever callback is doing
 * the most work — so the hook blocking the thread dilutes its own credit while cheap hooks
 * collect credit for waiting. Ranking from those numbers reliably puts the *cheapest* hook
 * first.
 *
 * The missing quantity is when the callback began running, and that is known here. Starting
 * the clock inside the callback removes the dispatch queue by construction.
 *
 * ## `maxInFlight` decides whether a total means anything
 *
 * Entry-to-exit spans may be summed only if they never overlap. A synchronous callback
 * cannot overlap: it holds the thread until it returns. An `async` one can — it may suspend
 * at an `await` and let another call of the same hook begin, and then both spans cover the
 * same wall clock and the sum counts it twice.
 *
 * Overlap also changes what the span is measuring. A hook that awaits the bundler (say
 * `this.resolve`) spends most of its span waiting for Rust, so the number describes the
 * bundler rather than the plugin. Observed on a real build: one `resolveId` accumulated
 * ~47,000s of span inside a 44s module-loading window.
 *
 * So overlap is counted rather than assumed, per hook:
 *
 * - `maxInFlight === 1` — no call ever overlapped another. The sum **is** execution time.
 * - `maxInFlight > 1` — spans overlap; the total is an upper bound and is not presented as a
 *   cost. The hook is named as needing a sampling profiler instead.
 *
 * This keeps the rule the core followed — never present a number that cannot be defended —
 * while narrowing "cannot measure" from a property of the *call site* to a property of the
 * *callback*, which is where it actually lives. A hook dispatched concurrently but whose
 * body never overlaps is measured exactly.
 *
 * A measured row is still wall time for that callback rather than CPU: a hook that awaits
 * I/O without ever overlapping another call of itself is charged for the wait.
 */

/** Nothing is reported below this, so ordinary fast builds stay quiet. */
const MIN_WINDOW_MS = 3_000;
/**
 * Nothing is reported unless plugin JavaScript held the thread for at least this share of
 * the window. This is the "are plugins why the build is slow?" question asked directly,
 * rather than inferred from how much of the build was not the link stage.
 */
const MIN_BUSY_SHARE = 0.2;
/** A row must reach this to be worth a line. */
const MIN_ROW_MS = 1_000;
const MAX_MEASURED_ROWS = 12;
const MAX_UNMEASURED_ROWS = 8;

export const PLUGIN_TIMINGS = 'PLUGIN_TIMINGS';

/** The owner shown for user callbacks configured on the output options, not on a plugin. */
export const OUTPUT_OPTIONS_OWNER = 'output options';

interface HookCost {
  owner: string;
  hookName: string;
  calls: number;
  ms: number;
  inFlight: number;
  maxInFlight: number;
}

export interface PluginTimingsRecorder {
  costs: Map<string, HookCost>;
  /**
   * First entry and last exit of any measured callback, i.e. the window they ran in.
   * `windowStart` is `-1` until the first call, since `0` is a timestamp a callback can
   * legitimately start at.
   */
  windowStart: number;
  windowEnd: number;
  /**
   * Wall time during which *any* measured callback was running — the union of every span,
   * so overlapping calls are counted once. Unlike a sum of spans this cannot exceed the
   * window, which is what makes it usable as a share.
   */
  busyMs: number;
  /** Across all hooks, for maintaining {@link PluginTimingsRecorder.busyMs}. */
  inFlight: number;
  busyStart: number;
  logHandler: LogHandler;
}

/**
 * One recorder per build, keyed on the object that identifies it.
 *
 * Counters cannot be module-global. A plugin is free to run a nested `rolldown()` build of
 * its own, which finishes first; shared counters would let it flush the outer build's
 * half-accumulated totals and report a window belonging to neither.
 */
const recorders = new WeakMap<object, PluginTimingsRecorder>();

/** The recorder for one build, created on first use. */
export function pluginTimingsRecorderFor(key: object, onLog: LogHandler): PluginTimingsRecorder {
  let recorder = recorders.get(key);
  if (recorder === undefined) {
    recorder = {
      costs: new Map(),
      windowStart: -1,
      windowEnd: 0,
      busyMs: 0,
      inFlight: 0,
      busyStart: 0,
      logHandler: onLog,
    };
    recorders.set(key, recorder);
  }
  return recorder;
}

function costFor(recorder: PluginTimingsRecorder, owner: string, hookName: string): HookCost {
  const key = `${owner} ${hookName}`;
  let cost = recorder.costs.get(key);
  if (cost === undefined) {
    cost = { owner, hookName, calls: 0, ms: 0, inFlight: 0, maxInFlight: 0 };
    recorder.costs.set(key, cost);
  }
  return cost;
}

/**
 * Wrap one callback so its execution time is recorded.
 *
 * Returns the callback untouched when there is no recorder, so a build that is not
 * measuring pays nothing beyond one branch at setup.
 */
export function measureHookCost<T extends (...args: never[]) => unknown>(
  recorder: PluginTimingsRecorder | undefined,
  owner: string,
  hookName: string,
  handler: T,
): T {
  if (recorder === undefined) {
    return handler;
  }

  const cost = costFor(recorder, owner, hookName);

  return function (this: unknown, ...args: never[]) {
    cost.calls += 1;
    cost.inFlight += 1;
    if (cost.inFlight > cost.maxInFlight) {
      cost.maxInFlight = cost.inFlight;
    }
    const started = performance.now();
    if (recorder.inFlight === 0) {
      recorder.busyStart = started;
    }
    recorder.inFlight += 1;
    if (recorder.windowStart < 0) {
      recorder.windowStart = started;
    }

    const finish = () => {
      const ended = performance.now();
      recorder.windowEnd = ended;
      cost.ms += ended - started;
      cost.inFlight -= 1;
      recorder.inFlight -= 1;
      if (recorder.inFlight === 0) {
        recorder.busyMs += ended - recorder.busyStart;
      }
    };

    let result: unknown;
    try {
      result = handler.apply(this, args);
    } catch (error) {
      finish();
      throw error;
    }

    // Only a thenable defers completion; a plain return value is already done, and treating
    // it as pending would leave `inFlight` permanently raised.
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
 * Render the report for one recorder, or `undefined` when there is nothing worth saying.
 *
 * Separate from {@link flushPluginTimings} so it can be tested without a multi-second build.
 */
export function renderPluginTimings(recorder: PluginTimingsRecorder): string | undefined {
  if (recorder.windowStart < 0) {
    return undefined;
  }
  const windowMs = recorder.windowEnd - recorder.windowStart;
  if (windowMs < MIN_WINDOW_MS || recorder.busyMs < windowMs * MIN_BUSY_SHARE) {
    return undefined;
  }

  const collected = [...recorder.costs.values()];
  const measured = collected
    .filter((cost) => cost.maxInFlight === 1 && cost.ms >= MIN_ROW_MS)
    .sort((a, b) => b.ms - a.ms);
  // Ordered by how heavily they overlapped: the worst offenders are the ones whose spans are
  // least meaningful, and the most likely to be worth profiling.
  const unmeasured = collected
    .filter((cost) => cost.maxInFlight > 1)
    .sort((a, b) => b.maxInFlight - a.maxInFlight);

  if (measured.length === 0 && unmeasured.length === 0) {
    return undefined;
  }

  const share = (ms: number) => Math.round((ms / windowMs) * 100);
  const lines = [
    `Your build spent ${share(recorder.busyMs)}% of a ${formatDuration(windowMs)} window ` +
      `inside plugin hooks (${formatDuration(recorder.busyMs)}).`,
  ];

  if (measured.length > 0) {
    lines.push('Measured inside the callbacks, so this is time they ran rather than waited:');
    for (const cost of measured.slice(0, MAX_MEASURED_ROWS)) {
      lines.push(
        `  - ${cost.owner} ${cost.hookName} ` +
          `(${share(cost.ms)}%, ${formatDuration(cost.ms)}, ${cost.calls} calls)`,
      );
    }
  }

  if (unmeasured.length > 0) {
    lines.push(
      'Not measurable — these callbacks overlap each other, so their elapsed time covers work',
      'other calls were doing. A hook awaiting the bundler measures the bundler, not itself.',
      'Profile them with `node --cpu-prof`:',
    );
    for (const cost of unmeasured.slice(0, MAX_UNMEASURED_ROWS)) {
      lines.push(`  - ${cost.owner} ${cost.hookName} (${cost.calls} calls)`);
    }
  }

  lines.push(
    'See https://rolldown.rs/reference/InputOptions.checks#plugintimings for more details.',
  );
  return lines.join('\n');
}

/**
 * Emit the report for one build, then drop its recorder. Safe to call when nothing was
 * measured, and when called more than once for the same build.
 */
export function flushPluginTimings(key: object): void {
  const recorder = recorders.get(key);
  if (recorder === undefined) {
    return;
  }
  recorders.delete(key);

  const message = renderPluginTimings(recorder);
  if (message !== undefined) {
    recorder.logHandler(LOG_LEVEL_WARN, { code: PLUGIN_TIMINGS, message });
  }
}
