import { afterEach, describe, expect, it, vi } from 'vitest';
import type { BindingChunkingContext, BindingMatchGroup } from '../src/binding.cjs';
import type { InputOptions } from '../src/options/input-options';
import type { CodeSplittingGroup, OutputOptions } from '../src/options/output-options';
import { createBundlerOptions } from '../src/utils/create-bundler-option';
import { pluginTimingsRecorderFor, summarizePluginTimings } from '../src/utils/plugin-timings';

vi.mock('../src/binding.cjs', async () => {
  const { loadBinding } = await import('./src/load-binding');
  return loadBinding();
});

function fakeClock() {
  let now = 1_000;
  vi.spyOn(performance, 'now').mockImplementation(() => now);
  return {
    advance(ms: number) {
      now += ms;
    },
  };
}

function getBindingGroups(
  bundlerOptions: Awaited<ReturnType<typeof createBundlerOptions>>['bundlerOptions'],
): BindingMatchGroup[] {
  const groups = bundlerOptions.outputOptions.manualCodeSplitting?.groups;
  if (groups === undefined) {
    throw new Error('Expected code-splitting groups');
  }
  return groups;
}

function runTest(group: BindingMatchGroup): void {
  if (typeof group.test !== 'function') {
    throw new Error('Expected a code-splitting test callback');
  }
  group.test(['entry.js']);
}

// The native side mints a fresh context box per batch, so every `runName` call
// gets its own fake. On the threadless-WASI flavor `getChunkingContext`
// (`src/utils/bindingify-output-options.ts`) releases the box it is replacing
// through `releaseOrDefer`, which calls `dropInner()` on it, so the fake has to
// answer that call. Native and threaded-WASI builds leave the release to GC
// finalizers and never touch it. `getModuleInfo` is the only other member a
// group callback can reach through the box.
function fakeChunkingContext(): BindingChunkingContext {
  return {
    dropInner: () => ({ freed: true }),
    getModuleInfo: () => null,
  };
}

function runName(group: BindingMatchGroup): void {
  if (typeof group.name !== 'function') {
    throw new Error('Expected a code-splitting name callback');
  }
  group.name(['entry.js'], fakeChunkingContext());
}

afterEach(() => {
  vi.restoreAllMocks();
});

describe('code-splitting group timings', () => {
  it('keeps groups with the same label separate across repeated outputs', async () => {
    const clock = fakeClock();
    const inputOptions: InputOptions = { checks: { bundlerTimings: true } };
    const firstGroup: CodeSplittingGroup = {
      name: 'shared',
      test: () => {
        clock.advance(10);
        return true;
      },
    };
    const secondGroup: CodeSplittingGroup = {
      name: 'shared',
      test: () => {
        clock.advance(30);
        return true;
      },
    };
    const outputOptions: OutputOptions = {
      codeSplitting: { groups: [firstGroup, secondGroup] },
    };

    for (let index = 0; index < 2; index++) {
      const { bundlerOptions } = await createBundlerOptions(
        inputOptions,
        outputOptions,
        false,
        undefined,
        undefined,
        undefined,
        true,
      );
      const groups = getBindingGroups(bundlerOptions);
      runTest(groups[0]);
      runTest(groups[1]);
    }

    const recorder = pluginTimingsRecorderFor(inputOptions);
    expect(recorder.costs.size).toBe(2);
    expect(
      recorder.costs.get(firstGroup)?.get('codeSplitting groups[0].test "shared"'),
    ).toMatchObject({
      calls: 2,
      ms: 20,
    });
    expect(
      recorder.costs.get(secondGroup)?.get('codeSplitting groups[1].test "shared"'),
    ).toMatchObject({
      calls: 2,
      ms: 60,
    });
  });

  it('tells two groups apart by position', async () => {
    const clock = fakeClock();
    const inputOptions: InputOptions = { checks: { bundlerTimings: true } };
    const slow: CodeSplittingGroup = {
      name: () => {
        clock.advance(40);
        return 'shared';
      },
    };
    const fast: CodeSplittingGroup = {
      name: () => {
        clock.advance(10);
        return 'shared';
      },
    };
    const { bundlerOptions } = await createBundlerOptions(
      inputOptions,
      { codeSplitting: { groups: [slow, fast] } },
      false,
      undefined,
      undefined,
      undefined,
      true,
    );
    const groups = getBindingGroups(bundlerOptions);
    runName(groups[0]);
    runName(groups[1]);

    // Both groups carry the same label, so the position is what separates the rows.
    expect(summarizePluginTimings(inputOptions).rows.map((row) => row.hook)).toEqual([
      'codeSplitting groups[0].name',
      'codeSplitting groups[1].name',
    ]);
  });

  it('keeps the test and name rows for one group', async () => {
    const clock = fakeClock();
    const inputOptions: InputOptions = { checks: { bundlerTimings: true } };
    const group: CodeSplittingGroup = {
      debugName: 'dynamic group',
      name: () => {
        clock.advance(20);
        return 'shared';
      },
      test: () => {
        clock.advance(10);
        return true;
      },
    };
    const { bundlerOptions } = await createBundlerOptions(
      inputOptions,
      { codeSplitting: { groups: [group] } },
      false,
      undefined,
      undefined,
      undefined,
      true,
    );
    const [bindingGroup] = getBindingGroups(bundlerOptions);

    runTest(bindingGroup);
    runName(bindingGroup);

    expect(pluginTimingsRecorderFor(inputOptions).costs.get(group)?.size).toBe(2);
    expect(summarizePluginTimings(inputOptions)).toMatchObject({
      busyMs: 30,
      rows: [
        { hook: 'codeSplitting groups[0].test "dynamic group"', calls: 1, ms: 10 },
        { hook: 'codeSplitting groups[0].name "dynamic group"', calls: 1, ms: 20 },
      ],
    });
  });

  it('uses debugName before a string name', async () => {
    const inputOptions: InputOptions = { checks: { bundlerTimings: true } };
    const group: CodeSplittingGroup = {
      debugName: 'report label',
      name: 'chunk name',
      test: () => true,
    };
    const { bundlerOptions } = await createBundlerOptions(
      inputOptions,
      { codeSplitting: { groups: [group] } },
      false,
      undefined,
      undefined,
      undefined,
      true,
    );
    const [bindingGroup] = getBindingGroups(bundlerOptions);

    expect(bindingGroup).not.toHaveProperty('debugName');
    expect(bindingGroup.name).toBe('chunk name');
    runTest(bindingGroup);

    expect(summarizePluginTimings(inputOptions).rows).toMatchObject([
      { hook: 'codeSplitting groups[0].test "report label"' },
    ]);
  });

  it('warns once and uses the group index when a dynamic name has no label', async () => {
    const warnings: Array<{ code?: string; message: string }> = [];
    const inputOptions: InputOptions = {
      checks: { bundlerTimings: true },
      onLog(level, log) {
        if (level === 'warn') warnings.push(log);
      },
    };
    const group: CodeSplittingGroup = {
      name: () => 'shared',
    };
    const outputOptions: OutputOptions = {
      codeSplitting: { groups: [group] },
    };

    for (let index = 0; index < 2; index++) {
      const { bundlerOptions } = await createBundlerOptions(
        inputOptions,
        outputOptions,
        false,
        undefined,
        undefined,
        undefined,
        true,
      );
      runName(getBindingGroups(bundlerOptions)[0]);
    }

    expect(warnings).toHaveLength(1);
    expect(warnings[0]).toMatchObject({
      code: 'MISSING_CODE_SPLITTING_GROUP_DEBUG_NAME',
      message:
        '`output.codeSplitting.groups[0].name` is a function. Set `output.codeSplitting.groups[0].debugName` so the bundler timing report can identify this group.',
    });
    expect(summarizePluginTimings(inputOptions).rows).toMatchObject([
      { hook: 'codeSplitting groups[0].name', calls: 2 },
    ]);
  });

  it('uses the advancedChunks path in its warning and timing row', async () => {
    const warnings: Array<{ code?: string; message: string }> = [];
    const inputOptions: InputOptions = {
      checks: { bundlerTimings: true },
      onLog(level, log) {
        if (level === 'warn') warnings.push(log);
      },
    };
    const group: CodeSplittingGroup = {
      name: () => 'shared',
    };
    const { bundlerOptions } = await createBundlerOptions(
      inputOptions,
      { advancedChunks: { groups: [group] } },
      false,
      undefined,
      undefined,
      undefined,
      true,
    );

    runName(getBindingGroups(bundlerOptions)[0]);

    expect(warnings).toHaveLength(1);
    expect(warnings[0]).toMatchObject({
      message:
        '`output.advancedChunks.groups[0].name` is a function. Set `output.advancedChunks.groups[0].debugName` so the bundler timing report can identify this group.',
    });
    expect(summarizePluginTimings(inputOptions).rows).toMatchObject([
      { hook: 'advancedChunks groups[0].name' },
    ]);
  });

  it('does not warn when timing is disabled', async () => {
    const warnings: Array<{ code?: string; message: string }> = [];
    const inputOptions: InputOptions = {
      checks: { bundlerTimings: false },
      onLog(level, log) {
        if (level === 'warn') warnings.push(log);
      },
    };
    await createBundlerOptions(
      inputOptions,
      { codeSplitting: { groups: [{ name: () => 'shared' }] } },
      false,
      undefined,
      undefined,
      undefined,
      true,
    );

    expect(warnings).toEqual([]);
  });

  it('keeps one manualChunks row across repeated outputs', async () => {
    const clock = fakeClock();
    const warnings: Array<{ code?: string; message: string }> = [];
    const inputOptions: InputOptions = {
      checks: { bundlerTimings: true },
      onLog(level, log) {
        if (level === 'warn') warnings.push(log);
      },
    };
    const manualChunks: NonNullable<OutputOptions['manualChunks']> = () => {
      clock.advance(600);
      return 'shared';
    };
    const outputOptions: OutputOptions = { manualChunks };

    for (let index = 0; index < 2; index++) {
      const { bundlerOptions } = await createBundlerOptions(
        inputOptions,
        outputOptions,
        false,
        undefined,
        undefined,
        undefined,
        true,
      );
      const [bindingGroup] = getBindingGroups(bundlerOptions);
      runName(bindingGroup);
    }

    const recorder = pluginTimingsRecorderFor(inputOptions);
    expect(recorder.costs.size).toBe(1);
    expect(recorder.costs.get(manualChunks)?.get('manualChunks')).toMatchObject({
      calls: 2,
      ms: 1_200,
    });
    expect(warnings).toEqual([]);
  });
});
