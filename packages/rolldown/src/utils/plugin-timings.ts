/**
 * Measure what plugin hooks cost, from inside the JavaScript callback.
 *
 * Internal to Rolldown. Nothing here is on a public entry point, and the measurement is not
 * offered as an API — the only consumer is the core, which pulls it while the build closes.
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
 * So overlap is measured rather than assumed, per hook: `overlapMs` is the wall time
 * double counted in `ms` because two or more calls were in flight together. A hook is
 * reported with a number when that is a negligible share of its span, and named without one
 * when it is not.
 *
 * The test is deliberately a tolerance rather than "did this ever happen". A peak in-flight
 * count of 2 can mean one incidental overlap in twenty thousand calls, and discarding a
 * twelve-second measurement over that both throws away good data and makes the report
 * depend on scheduling luck — the same hook would appear in one run and vanish in the next.
 *
 * This keeps the rule the core followed — never present a number that cannot be defended —
 * while narrowing "cannot measure" from a property of the *call site* to a property of the
 * *callback*, which is where it actually lives. A hook dispatched concurrently but whose
 * body barely overlaps is measured to within the tolerance.
 *
 * A measured row is still wall time for that callback rather than CPU: a hook that awaits
 * I/O without overlapping another call of itself is charged for the wait. Excluding the
 * dispatch queue is what this buys; it does not distinguish running from awaiting.
 */

/**
 * How much of a hook's span may be double counted before its total stops being reportable,
 * as a fraction of that span.
 */
const OVERLAP_TOLERANCE = 0.01;

/**
 * Who a measured callback belongs to.
 *
 * `key` and `name` are separate on purpose. Two configured instances of one plugin share a
 * name — normalization allows it — but are different culprits, and merging them would also
 * let one instance's overlap mark the other's clean measurement unrankable. So rows are
 * keyed on identity and the name is only ever displayed.
 */
export interface TimingOwner {
  /** Unique per configured instance, and stable across a build's outputs. */
  key: unknown;
  /** What the report shows. */
  name: string;
  kind: PluginTimingKind;
}

/**
 * Where a callback was configured. Supplied by the wrapping call site, which knows: it
 * cannot be recovered from the owner's name, since a plugin may legally be called
 * `output options`.
 */
export type PluginTimingKind = 'plugin' | 'outputOption' | 'inputOption';

interface HookCost {
  owner: string;
  kind: PluginTimingKind;
  hookName: string;
  calls: number;
  ms: number;
  inFlight: number;
  maxInFlight: number;
  /**
   * Wall time counted more than once in `ms`, because that many calls of this hook were in
   * flight over the same interval. `ms - overlapMs` is the time the hook actually occupied.
   */
  overlapMs: number;
  /** When `inFlight` last changed, for maintaining `overlapMs`. */
  lastChange: number;
}

export interface PluginTimingsRecorder {
  /**
   * Owner identity, then hook. Nested rather than keyed on a joined string because plugin
   * names are user-controlled: any separator is a name a plugin may legally have, and a
   * collision would silently merge two plugins' rows.
   */
  costs: Map<unknown, Map<string, HookCost>>;
  /**
   * Wall time during which *any* measured callback was running — the union of every span,
   * so overlapping calls are counted once. Unlike a sum of spans this cannot outrun the
   * build, which is what makes it usable as a share.
   */
  busyMs: number;
  /** Across all hooks, for maintaining {@link PluginTimingsRecorder.busyMs}. */
  inFlight: number;
  busyStart: number;
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
export function pluginTimingsRecorderFor(key: object): PluginTimingsRecorder {
  let recorder = recorders.get(key);
  if (recorder === undefined) {
    recorder = {
      costs: new Map(),
      busyMs: 0,
      inFlight: 0,
      busyStart: 0,
    };
    recorders.set(key, recorder);
  }
  return recorder;
}

function costFor(recorder: PluginTimingsRecorder, owner: TimingOwner, hookName: string): HookCost {
  let byHook = recorder.costs.get(owner.key);
  if (byHook === undefined) {
    byHook = new Map();
    recorder.costs.set(owner.key, byHook);
  }
  let cost = byHook.get(hookName);
  if (cost === undefined) {
    cost = {
      owner: owner.name,
      kind: owner.kind,
      hookName,
      calls: 0,
      ms: 0,
      inFlight: 0,
      maxInFlight: 0,
      overlapMs: 0,
      lastChange: 0,
    };
    byHook.set(hookName, cost);
  }
  return cost;
}

/**
 * Fold the interval since the last in-flight change into `overlapMs`, then move the mark.
 * Call immediately before every increment and decrement of `cost.inFlight`.
 */
function markInFlightChange(cost: HookCost, at: number): void {
  if (cost.inFlight > 1) {
    // All `inFlight` spans accrued over this interval, so all but one of them double count it.
    cost.overlapMs += (cost.inFlight - 1) * (at - cost.lastChange);
  }
  cost.lastChange = at;
}

/**
 * Close out one call. A module-level function rather than a closure per invocation: hooks
 * run hundreds of thousands of times in a large build, and the synchronous path would
 * otherwise allocate one closure per call only to invoke it immediately.
 */
function settle(recorder: PluginTimingsRecorder, cost: HookCost, started: number): void {
  const ended = performance.now();
  markInFlightChange(cost, ended);
  cost.inFlight -= 1;
  cost.ms += ended - started;
  recorder.inFlight -= 1;
  if (recorder.inFlight === 0) {
    recorder.busyMs += ended - recorder.busyStart;
  }
}

/**
 * Wrap one callback so its execution time is recorded.
 *
 * Returns the callback untouched when there is no recorder, so a build that is not
 * measuring pays nothing beyond one branch at setup.
 */
export function measureHookCost<T extends (...args: never[]) => unknown>(
  recorder: PluginTimingsRecorder | undefined,
  owner: TimingOwner,
  hookName: string,
  handler: T,
): T {
  if (recorder === undefined) {
    return handler;
  }

  const cost = costFor(recorder, owner, hookName);

  return function (this: unknown, ...args: never[]) {
    const started = performance.now();
    markInFlightChange(cost, started);
    cost.calls += 1;
    cost.inFlight += 1;
    if (cost.inFlight > cost.maxInFlight) {
      cost.maxInFlight = cost.inFlight;
    }
    if (recorder.inFlight === 0) {
      recorder.busyStart = started;
    }
    recorder.inFlight += 1;

    let result: unknown;
    try {
      result = handler.apply(this, args);
    } catch (error) {
      settle(recorder, cost, started);
      throw error;
    }

    // Only a thenable defers completion; a plain return value is already done, and treating
    // it as pending would leave `inFlight` permanently raised.
    if (typeof (result as { then?: unknown } | undefined)?.then === 'function') {
      return (result as Promise<unknown>).then(
        (value) => {
          settle(recorder, cost, started);
          return value;
        },
        (error: unknown) => {
          settle(recorder, cost, started);
          throw error;
        },
      );
    }

    settle(recorder, cost, started);
    return result;
  } as T;
}

/**
 * The owner shown for user callbacks the core invokes directly rather than through a
 * plugin. One shared identity: they are all configured on the same options object.
 */
export const OUTPUT_OPTIONS_OWNER: TimingOwner = {
  key: Symbol('output options'),
  name: 'output options',
  kind: 'outputOption',
};

/** As {@link OUTPUT_OPTIONS_OWNER}, for callbacks configured on the input options. */
export const INPUT_OPTIONS_OWNER: TimingOwner = {
  key: Symbol('input options'),
  name: 'input options',
  kind: 'inputOption',
};

/**
 * Wrap `value` when the user supplied a function, and leave every other form alone.
 *
 * Most option callbacks are declared as `string | RegExp | Function | ...`, so this keeps
 * the one cast the wrapping needs in a single place.
 */
export function measureIfFunction<T>(
  recorder: PluginTimingsRecorder | undefined,
  owner: TimingOwner,
  hookName: string,
  value: T,
): T {
  if (typeof value !== 'function') {
    return value;
  }
  return measureHookCost(
    recorder,
    owner,
    hookName,
    value as unknown as (...args: never[]) => unknown,
  ) as unknown as T;
}

/** What one owner's hook cost the build. */
export interface PluginTimingRow {
  /** The plugin the callback belongs to, or the options it was configured on. */
  owner: string;
  kind: PluginTimingKind;
  /** `transform`, `codeSplitting groups[].name`, … — what the user wrote in their config. */
  hook: string;
  calls: number;
  /** Summed spans. Meaningful as a cost only when {@link PluginTimingRow.rankable}. */
  ms: number;
  /** Peak concurrent calls. Informational — {@link PluginTimingRow.rankable} is the verdict. */
  maxInFlight: number;
  /** How much of {@link PluginTimingRow.ms} is double counted, in ms. */
  overlapMs: number;
  /**
   * Whether {@link PluginTimingRow.ms} may be compared against another row's. False when
   * enough of this hook's calls overlapped that its total meaningfully double counts wall
   * clock; a hook that overlapped itself once in twenty thousand calls stays comparable.
   */
  rankable: boolean;
}

/**
 * What this side measured, and only that.
 *
 * The build and link-stage clocks are not here: they are visible from Rust and never leave
 * it, so the sink that renders joins the two halves rather than one side accumulating the
 * other's numbers.
 *
 * Plain JSON — no `Map`, no closures, stable field names — because it crosses the binding
 * as `BindingPluginTimingsMeasurement`.
 */
export interface PluginTimingsMeasurement {
  /** Union of every span, so it counts overlap once and cannot outrun the build. */
  busyMs: number;
  /**
   * First-call order. No significance — ordering is the renderer's call, and this side does
   * not know which of them is rendering.
   */
  rows: PluginTimingRow[];
}

/**
 * What the build's callbacks have cost so far, leaving the recorder in place.
 *
 * Ungated on purpose: whether a build is worth reporting on is decided from the clocks Rust
 * holds, so the question is not this side's to ask.
 *
 * Rust calls this while the build is closing, so what it gets includes `closeBundle`.
 */
export function summarizePluginTimings(key: object): PluginTimingsMeasurement {
  const recorder = recorders.get(key);
  if (recorder === undefined) {
    return { busyMs: 0, rows: [] };
  }

  const rows: PluginTimingRow[] = [];
  for (const byHook of recorder.costs.values()) {
    for (const cost of byHook.values()) {
      // A row exists as soon as a callback is wrapped, which happens whether or not the core
      // ever calls it — `renderError` on a build that succeeded, say. Nothing ran, so there
      // is nothing for any consumer to say about it.
      if (cost.calls === 0) {
        continue;
      }
      rows.push({
        owner: cost.owner,
        kind: cost.kind,
        hook: cost.hookName,
        calls: cost.calls,
        ms: cost.ms,
        maxInFlight: cost.maxInFlight,
        overlapMs: cost.overlapMs,
        rankable: cost.overlapMs <= cost.ms * OVERLAP_TOLERANCE,
      });
    }
  }

  return { busyMs: recorder.busyMs, rows };
}
