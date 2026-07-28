import { afterEach, describe, expect, it, vi } from 'vitest';
import {
  flushPluginTimings,
  measureHookCost,
  PLUGIN_TIMINGS,
  type PluginTimingsRecorder,
  pluginTimingsRecorderFor,
  renderPluginTimings,
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
  return pluginTimingsRecorderFor({}, () => {});
}

function costOf(recorder: PluginTimingsRecorder, key: string) {
  const cost = recorder.costs.get(key);
  if (cost === undefined) {
    throw new Error(`no row for ${key}, have ${[...recorder.costs.keys()].join(', ')}`);
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
    const hook = measureHookCost(recorder, 'my-plugin', 'transform', () => {
      clock.advance(40);
      return 'done';
    });

    expect(hook()).toBe('done');

    const cost = costOf(recorder, 'my-plugin transform');
    expect(cost.ms).toBe(40);
    expect(cost.calls).toBe(1);
    expect(cost.maxInFlight).toBe(1);
    expect(cost.inFlight).toBe(0);
  });

  it('returns the handler untouched when nothing is recording', () => {
    const handler = () => 'x';
    expect(measureHookCost(undefined, 'my-plugin', 'transform', handler)).toBe(handler);
  });

  it('settles when the callback throws, and rethrows', () => {
    const clock = fakeClock();
    const recorder = newRecorder();
    const boom = new Error('boom');
    const hook = measureHookCost(recorder, 'my-plugin', 'load', () => {
      clock.advance(10);
      throw boom;
    });

    expect(hook).toThrow(boom);

    const cost = costOf(recorder, 'my-plugin load');
    expect(cost.ms).toBe(10);
    // A leaked count would make every later call look like it overlapped this one.
    expect(cost.inFlight).toBe(0);
    expect(recorder.inFlight).toBe(0);
  });

  it('settles when the returned promise rejects, and rejects', async () => {
    const recorder = newRecorder();
    const boom = new Error('boom');
    const hook = measureHookCost(recorder, 'my-plugin', 'load', () => Promise.reject(boom));

    await expect(hook()).rejects.toBe(boom);
    expect(costOf(recorder, 'my-plugin load').inFlight).toBe(0);
    expect(recorder.inFlight).toBe(0);
  });

  it('does not treat a plain return value as pending', () => {
    const recorder = newRecorder();
    const hook = measureHookCost(recorder, 'my-plugin', 'resolveId', () => ({ id: 'x' }));

    hook();
    hook();

    // Were a non-thenable mistaken for a pending call, `inFlight` would climb and the hook
    // would be written off as overlapping when it never was.
    expect(costOf(recorder, 'my-plugin resolveId').maxInFlight).toBe(1);
  });

  it('keeps sequential async calls measurable and sums them', async () => {
    const clock = fakeClock();
    const recorder = newRecorder();
    const hook = measureHookCost(recorder, 'my-plugin', 'transform', async () => {
      clock.advance(30);
    });

    await hook();
    await hook();

    const cost = costOf(recorder, 'my-plugin transform');
    expect(cost.maxInFlight).toBe(1);
    expect(cost.ms).toBe(60);
  });

  it('marks a hook whose calls overlap', async () => {
    const recorder = newRecorder();
    let release!: () => void;
    const gate = new Promise<void>((resolve) => {
      release = resolve;
    });
    const hook = measureHookCost(recorder, 'my-plugin', 'transform', () => gate);

    const both = Promise.all([hook(), hook()]);
    // Both callbacks are suspended at the same await, which is exactly the case whose
    // spans cover the same wall clock and must not be summed.
    expect(costOf(recorder, 'my-plugin transform').maxInFlight).toBe(2);
    release();
    await both;
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
    const first = measureHookCost(recorder, 'a-plugin', 'transform', () => a)();
    clock.set(10);
    const second = measureHookCost(recorder, 'b-plugin', 'transform', () => b)();
    clock.set(30);
    releaseA();
    await first;
    clock.set(40);
    releaseB();
    await second;

    // Spans are [0,30] and [10,40] — 60ms summed, but only 40ms of wall clock was ever
    // spent inside a callback, and a share of the window has to be the latter.
    expect(
      costOf(recorder, 'a-plugin transform').ms + costOf(recorder, 'b-plugin transform').ms,
    ).toBe(60);
    expect(recorder.busyMs).toBe(40);
    expect(recorder.windowStart).toBe(0);
    expect(recorder.windowEnd).toBe(40);
  });
});

describe('renderPluginTimings', () => {
  function recorderWith(
    partial: Partial<PluginTimingsRecorder>,
    rows: Array<[string, string, { ms: number; calls: number; maxInFlight: number }]> = [],
  ): PluginTimingsRecorder {
    const recorder: PluginTimingsRecorder = {
      costs: new Map(),
      windowStart: 0,
      windowEnd: 10_000,
      busyMs: 8_000,
      inFlight: 0,
      busyStart: 0,
      logHandler: () => {},
      ...partial,
    };
    for (const [owner, hookName, row] of rows) {
      recorder.costs.set(`${owner} ${hookName}`, { owner, hookName, inFlight: 0, ...row });
    }
    return recorder;
  }

  it('stays quiet for a short build', () => {
    const recorder = recorderWith({ windowEnd: 2_000, busyMs: 2_000 }, [
      ['my-plugin', 'transform', { ms: 1_900, calls: 10, maxInFlight: 1 }],
    ]);
    expect(renderPluginTimings(recorder)).toBeUndefined();
  });

  it('stays quiet when plugins were not what the build was doing', () => {
    // A long build in which plugin JavaScript held the thread for 5% of the window is not
    // a plugin problem, however large the biggest row looks on its own.
    const recorder = recorderWith({ windowEnd: 60_000, busyMs: 3_000 }, [
      ['my-plugin', 'transform', { ms: 3_000, calls: 10, maxInFlight: 1 }],
    ]);
    expect(renderPluginTimings(recorder)).toBeUndefined();
  });

  it('ranks measured rows and names the ones it cannot measure', () => {
    const recorder = recorderWith({ windowEnd: 10_000, busyMs: 8_000 }, [
      ['slow-plugin', 'transform', { ms: 6_000, calls: 500, maxInFlight: 1 }],
      ['output options', 'codeSplitting groups[].name', { ms: 2_000, calls: 900, maxInFlight: 1 }],
      ['async-plugin', 'resolveId', { ms: 47_000, calls: 3_000, maxInFlight: 42 }],
      ['tiny-plugin', 'buildStart', { ms: 5, calls: 1, maxInFlight: 1 }],
    ]);

    const message = renderPluginTimings(recorder)!;
    const rows = message.split('\n').filter((line) => line.startsWith('  - '));

    expect(rows).toEqual([
      '  - slow-plugin transform (60%, 6.0s, 500 calls)',
      '  - output options codeSplitting groups[].name (20%, 2.0s, 900 calls)',
      '  - async-plugin resolveId (3000 calls)',
    ]);
    expect(message).toContain('80% of a 10.0s window');
    // The 47s row would top any ranking, which is exactly why it gets no number.
    expect(message).not.toContain('47.0s');
    // Below the one-second floor.
    expect(message).not.toContain('tiny-plugin');
  });

  it('says nothing when every row is below the floor', () => {
    const recorder = recorderWith({ windowEnd: 10_000, busyMs: 8_000 }, [
      ['tiny-plugin', 'buildStart', { ms: 5, calls: 1, maxInFlight: 1 }],
    ]);
    expect(renderPluginTimings(recorder)).toBeUndefined();
  });
});

describe('flushPluginTimings', () => {
  it('emits a warning under the plugin timings code, once', () => {
    const clock = fakeClock();
    const key = {};
    const logs: Array<{ level: string; code?: string; message?: string }> = [];
    const recorder = pluginTimingsRecorderFor(key, (level, log) => {
      logs.push({ level, ...log });
    });

    clock.set(0);
    const hook = measureHookCost(recorder, 'slow-plugin', 'transform', () => clock.advance(5_000));
    hook();

    flushPluginTimings(key);
    expect(logs).toHaveLength(1);
    expect(logs[0].level).toBe('warn');
    expect(logs[0].code).toBe(PLUGIN_TIMINGS);
    expect(logs[0].message).toContain('slow-plugin transform');

    // The recorder is dropped, so a second `close()` cannot re-report the same build.
    flushPluginTimings(key);
    expect(logs).toHaveLength(1);
  });

  it('says nothing for a build no recorder was ever created for', () => {
    expect(() => flushPluginTimings({})).not.toThrow();
  });
});
