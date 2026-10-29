import { afterEach, describe, expect, it, vi } from 'vitest';
import {
  measureHookCost,
  type PluginTimingKind,
  type PluginTimingRow,
  type PluginTimingsRecorder,
  OUTPUT_OPTIONS_OWNER,
  pluginTimingsRecorderFor,
  summarizePluginTimings,
} from '../src/utils/plugin-timings';

/**
 * Drive `performance.now()` from a script so the assertions are exact arithmetic rather
 * than a race with the machine.
 */
function fakeClock(): { advance: (ms: number) => void; set: (ms: number) => void } {
  let now = 1_000;
  vi.spyOn(performance, 'now').mockImplementation(() => now);
  return {
    advance: (ms) => {
      now += ms;
    },
    set: (ms) => {
      now = ms;
    },
  };
}

function newRecorder(): PluginTimingsRecorder {
  return pluginTimingsRecorderFor({});
}

function costOf(recorder: PluginTimingsRecorder, owner: unknown, hookName: string) {
  const cost = recorder.costs.get(owner)?.get(hookName);
  if (cost === undefined) {
    throw new Error(`no row for ${String(owner)} ${hookName}, have ${recorder.costs.size} owners`);
  }
  return cost;
}

afterEach(() => {
  vi.restoreAllMocks();
});

describe('measureHookCost', () => {
  it('charges a synchronous callback its own execution time', () => {
    const clock = fakeClock();
    const recorder = newRecorder();
    const hook = measureHookCost(
      recorder,
      { key: 'my-plugin', name: 'my-plugin', kind: 'plugin' },
      'transform',
      () => {
        clock.advance(40);
        return 'done';
      },
    );

    expect(hook()).toBe('done');

    const cost = costOf(recorder, 'my-plugin', 'transform');
    expect(cost.ms).toBe(40);
    expect(cost.calls).toBe(1);
    expect(cost.maxInFlight).toBe(1);
    expect(cost.inFlight).toBe(0);
  });

  it('returns the handler untouched when nothing is recording', () => {
    const handler = () => 'x';
    expect(
      measureHookCost(
        undefined,
        { key: 'my-plugin', name: 'my-plugin', kind: 'plugin' },
        'transform',
        handler,
      ),
    ).toBe(handler);
  });

  it('settles when the callback throws, and rethrows', () => {
    const clock = fakeClock();
    const recorder = newRecorder();
    const boom = new Error('boom');
    const hook = measureHookCost(
      recorder,
      { key: 'my-plugin', name: 'my-plugin', kind: 'plugin' },
      'load',
      () => {
        clock.advance(10);
        throw boom;
      },
    );

    expect(hook).toThrow(boom);

    const cost = costOf(recorder, 'my-plugin', 'load');
    expect(cost.ms).toBe(10);
    // A leaked count would make every later call look like it overlapped this one.
    expect(cost.inFlight).toBe(0);
    expect(recorder.inFlight).toBe(0);
  });

  it('settles when the returned promise rejects, and rejects', async () => {
    const recorder = newRecorder();
    const boom = new Error('boom');
    const hook = measureHookCost(
      recorder,
      { key: 'my-plugin', name: 'my-plugin', kind: 'plugin' },
      'load',
      () => Promise.reject(boom),
    );

    await expect(hook()).rejects.toBe(boom);
    expect(costOf(recorder, 'my-plugin', 'load').inFlight).toBe(0);
    expect(recorder.inFlight).toBe(0);
  });

  it('does not treat a plain return value as pending', () => {
    const recorder = newRecorder();
    const hook = measureHookCost(
      recorder,
      { key: 'my-plugin', name: 'my-plugin', kind: 'plugin' },
      'resolveId',
      () => ({ id: 'x' }),
    );

    hook();
    hook();

    // Were a non-thenable mistaken for a pending call, `inFlight` would climb and the hook
    // would be written off as overlapping when it never was.
    expect(costOf(recorder, 'my-plugin', 'resolveId').maxInFlight).toBe(1);
  });

  it('keeps sequential async calls measurable and sums them', async () => {
    const clock = fakeClock();
    const recorder = newRecorder();
    const hook = measureHookCost(
      recorder,
      { key: 'my-plugin', name: 'my-plugin', kind: 'plugin' },
      'transform',
      async () => {
        clock.advance(30);
      },
    );

    await hook();
    await hook();

    const cost = costOf(recorder, 'my-plugin', 'transform');
    expect(cost.maxInFlight).toBe(1);
    expect(cost.ms).toBe(60);
  });

  it('marks a hook whose calls overlap', async () => {
    const recorder = newRecorder();
    let release!: () => void;
    const gate = new Promise<void>((resolve) => {
      release = resolve;
    });
    const hook = measureHookCost(
      recorder,
      { key: 'my-plugin', name: 'my-plugin', kind: 'plugin' },
      'transform',
      () => gate,
    );

    const both = Promise.all([hook(), hook()]);
    // Both callbacks are suspended at the same await, which is exactly the case whose
    // spans cover the same wall clock and must not be summed.
    expect(costOf(recorder, 'my-plugin', 'transform').maxInFlight).toBe(2);
    release();
    await both;
  });
});

describe('owner identity', () => {
  it('keeps two instances of a same-named plugin apart', () => {
    const clock = fakeClock();
    const recorder = newRecorder();
    // `normalizePlugins` allows two configured instances to share a name. Keyed on the name
    // they would merge, and one instance's overlap would mark the other's clean measurement
    // unrankable.
    const first = { name: 'dup' };
    const second = { name: 'dup' };
    const a = measureHookCost(
      recorder,
      { key: first, name: 'dup', kind: 'plugin' },
      'transform',
      () => {
        clock.advance(10);
      },
    );
    const b = measureHookCost(
      recorder,
      { key: second, name: 'dup', kind: 'plugin' },
      'transform',
      () => {
        clock.advance(30);
      },
    );

    a();
    b();

    const summary = summarizePluginTimings({});
    expect(summary.rows).toHaveLength(0);
    expect(recorder.costs.size).toBe(2);
    expect(costOf(recorder, first, 'transform').ms).toBe(10);
    expect(costOf(recorder, second, 'transform').ms).toBe(30);
  });
});

describe('owner kind', () => {
  it('says where a callback was configured, so no consumer has to match on the name', () => {
    const clock = fakeClock();
    const recorder = newRecorder();
    measureHookCost(recorder, OUTPUT_OPTIONS_OWNER, 'codeSplitting groups[].name', () => {
      clock.advance(1);
    })();
    measureHookCost(recorder, { key: 'p', name: 'p', kind: 'plugin' }, 'transform', () => {
      clock.advance(1);
    })();

    const kinds = [...recorder.costs.values()].flatMap((byHook) =>
      [...byHook.values()].map((cost) => cost.kind),
    );
    const expected: PluginTimingKind[] = ['outputOption', 'plugin'];
    expect(kinds).toEqual(expected);
  });
});

describe('overlap accounting', () => {
  it("measures exactly how much of a hook's span is double counted", async () => {
    const clock = fakeClock();
    const recorder = newRecorder();
    let releaseA!: () => void;
    let releaseB!: () => void;
    const a = new Promise<void>((resolve) => {
      releaseA = resolve;
    });
    const b = new Promise<void>((resolve) => {
      releaseB = resolve;
    });
    const gates = [a, b];
    let next = 0;
    const hook = measureHookCost(
      recorder,
      { key: 'my-plugin', name: 'my-plugin', kind: 'plugin' },
      'transform',
      () => gates[next++]!,
    );

    clock.set(0);
    const first = hook();
    clock.set(10);
    const second = hook();
    clock.set(30);
    releaseA();
    await first;
    clock.set(40);
    releaseB();
    await second;

    // Spans [0,30] and [10,40] sum to 60ms over 40ms of wall clock, so exactly 20ms is
    // counted twice — which is what `ms - overlapMs` has to recover.
    const cost = costOf(recorder, 'my-plugin', 'transform');
    expect(cost.ms).toBe(60);
    expect(cost.overlapMs).toBe(20);
  });
});

describe('busy time', () => {
  it('counts wall clock in which any callback ran, not the sum of spans', async () => {
    const clock = fakeClock();
    const recorder = newRecorder();
    let releaseA!: () => void;
    let releaseB!: () => void;
    const a = new Promise<void>((resolve) => {
      releaseA = resolve;
    });
    const b = new Promise<void>((resolve) => {
      releaseB = resolve;
    });

    clock.set(0);
    const first = measureHookCost(
      recorder,
      { key: 'a-plugin', name: 'a-plugin', kind: 'plugin' },
      'transform',
      () => a,
    )();
    clock.set(10);
    const second = measureHookCost(
      recorder,
      { key: 'b-plugin', name: 'b-plugin', kind: 'plugin' },
      'transform',
      () => b,
    )();
    clock.set(30);
    releaseA();
    await first;
    clock.set(40);
    releaseB();
    await second;

    // Spans are [0,30] and [10,40] — 60ms summed, but only 40ms of wall clock was ever
    // spent inside a callback, and a share of the window has to be the latter.
    expect(
      costOf(recorder, 'a-plugin', 'transform').ms + costOf(recorder, 'b-plugin', 'transform').ms,
    ).toBe(60);
    expect(recorder.busyMs).toBe(40);
  });
});

/** Build a recorder holding exactly `rows`, and hand back what it measured. */
function summaryWith(
  rows: Array<
    [string, string, { ms: number; calls: number; maxInFlight: number; overlapMs?: number }]
  > = [],
  busyMs = 8_000,
) {
  const key = {};
  const recorder = pluginTimingsRecorderFor(key);
  recorder.busyMs = busyMs;
  for (const [owner, hookName, row] of rows) {
    let byHook = recorder.costs.get(owner);
    if (byHook === undefined) {
      byHook = new Map();
      recorder.costs.set(owner, byHook);
    }
    byHook.set(hookName, {
      owner,
      hookName,
      kind: 'plugin',
      inFlight: 0,
      lastChange: 0,
      overlapMs: 0,
      ...row,
    });
  }
  return summarizePluginTimings(key);
}

describe('summarizePluginTimings', () => {
  it('has nothing to say about a build it never watched', () => {
    expect(summarizePluginTimings({})).toEqual({ busyMs: 0, rows: [] });
  });

  it('reports the numbers whatever they say', () => {
    // Ungated on purpose: whether a build is worth reporting on is decided from clocks this
    // side does not hold, so it is not this side's question to ask.
    const summary = summaryWith(
      [['my-plugin', 'transform', { ms: 4, calls: 2, maxInFlight: 1 }]],
      4,
    );

    expect(summary.busyMs).toBe(4);
    const expected: PluginTimingRow[] = [
      {
        owner: 'my-plugin',
        hook: 'transform',
        calls: 2,
        ms: 4,
        kind: 'plugin',
        maxInFlight: 1,
        overlapMs: 0,
        rankable: true,
      },
    ];
    // Typed against the exported shape, so a field added or renamed without updating the
    // consumers that read it across the binding fails here.
    expect(summary.rows).toEqual(expected);
  });

  it('drops a callback the core never called', () => {
    // Wrapping creates the row, so a hook the build never needed — `renderError` on a build
    // that succeeded — would otherwise show up with 0 calls, and `maxInFlight` of 0 would
    // file it under "not measurable".
    const summary = summaryWith([
      ['quiet-plugin', 'renderError', { ms: 0, calls: 0, maxInFlight: 0 }],
      ['my-plugin', 'transform', { ms: 500, calls: 3, maxInFlight: 1 }],
    ]);

    expect(summary.rows.map((row) => row.hook)).toEqual(['transform']);
  });

  it('marks which rows carry a comparable number', () => {
    const summary = summaryWith([
      ['a', 'resolveId', { ms: 90_000, calls: 9, maxInFlight: 3, overlapMs: 80_000 }],
      ['b', 'transform', { ms: 100, calls: 1, maxInFlight: 1 }],
    ]);

    // 90s of span, most of it double counted, against a clean 100ms — and it is the 100ms
    // row that can be ranked. Ordering is left to whoever renders; this side only says
    // which numbers mean anything.
    expect(summary.rows.map((row) => [row.owner, row.rankable])).toEqual([
      ['a', false],
      ['b', true],
    ]);
  });

  it('keeps a measurement that overlapped itself only marginally', () => {
    // The case that made the report non-deterministic: one incidental overlap among many
    // calls used to discard the whole row, so a hook appeared in one run and vanished in
    // the next depending on scheduling. Judge the size of the overlap, not that it happened.
    const summary = summaryWith([
      ['tailwind', 'transform', { ms: 12_300, calls: 22, maxInFlight: 2, overlapMs: 40 }],
      ['heavy', 'transform', { ms: 12_300, calls: 22, maxInFlight: 2, overlapMs: 6_000 }],
    ]);

    expect(summary.rows.map((row) => [row.owner, row.rankable])).toEqual([
      ['tailwind', true],
      ['heavy', false],
    ]);
  });
});
