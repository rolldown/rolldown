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

function runName(group: BindingMatchGroup): void {
  if (typeof group.name !== 'function') {
    throw new Error('Expected a code-splitting name callback');
  }
  group.name(['entry.js'], { getModuleInfo: () => null } as BindingChunkingContext);
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
        true,
      );
      const groups = getBindingGroups(bundlerOptions);
      runTest(groups[0]);
      runTest(groups[1]);
    }

    const recorder = pluginTimingsRecorderFor(inputOptions);
    expect(recorder.costs.size).toBe(2);
    expect(recorder.costs.get(firstGroup)?.get('codeSplitting groups[].test')).toMatchObject({
      calls: 2,
      ms: 20,
    });
    expect(recorder.costs.get(secondGroup)?.get('codeSplitting groups[].test')).toMatchObject({
      calls: 2,
      ms: 60,
    });
  });

  it('keeps the test and name rows for one group', async () => {
    const clock = fakeClock();
    const inputOptions: InputOptions = { checks: { bundlerTimings: true } };
    const group: CodeSplittingGroup = {
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
      true,
    );
    const [bindingGroup] = getBindingGroups(bundlerOptions);

    runTest(bindingGroup);
    runName(bindingGroup);

    expect(pluginTimingsRecorderFor(inputOptions).costs.get(group)?.size).toBe(2);
    expect(summarizePluginTimings(inputOptions)).toMatchObject({
      busyMs: 30,
      rows: [
        { hook: 'codeSplitting groups[].test', calls: 1, ms: 10 },
        { hook: 'codeSplitting groups[].name', calls: 1, ms: 20 },
      ],
    });
  });
});
