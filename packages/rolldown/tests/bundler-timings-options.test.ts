import { setTimeout } from 'node:timers/promises';
import { describe, expect, it, vi } from 'vitest';
import { BindingBundler, shutdownAsyncRuntime, startAsyncRuntime } from '../src/binding.cjs';
import type { InputOptions } from '../src/options/input-options';
import { createBundlerOptions } from '../src/utils/create-bundler-option';
import { unwrapBindingResult } from '../src/utils/error';
import { validateCliOptions } from '../src/utils/validator';

vi.mock('../src/binding.cjs', async () => {
  const { loadBinding } = await import('./src/load-binding');
  return loadBinding();
});

const cases: [InputOptions['checks'], boolean][] = [
  [undefined, true],
  [{}, true],
  [{ bundlerTimings: true }, true],
  [{ bundlerTimings: false }, false],
  [{ pluginTimings: true }, true],
  [{ pluginTimings: false }, false],
  [{ bundlerTimings: true, pluginTimings: true }, true],
  [{ bundlerTimings: true, pluginTimings: false }, true],
  [{ bundlerTimings: false, pluginTimings: true }, false],
  [{ bundlerTimings: false, pluginTimings: false }, false],
];

describe('checks.bundlerTimings alias', () => {
  it.concurrent.each(cases)(
    'enables recording and filters native diagnostics with checks=%j: %s',
    async (checks, enabled) => {
      expect(validateCliOptions({ checks })[1]).toBeUndefined();
      const onLog = vi.fn();
      let beforeLinkStartedAt = 0;
      const { bundlerOptions } = await createBundlerOptions(
        {
          input: 'entry',
          checks,
          onLog,
          plugins: [
            {
              name: 'timing-check',
              resolveId: () => '\0entry',
              load: () => 'export default 1',
              buildEnd() {
                beforeLinkStartedAt = performance.now();
              },
              async renderStart() {
                const linkStageUpperBoundMs = performance.now() - beforeLinkStartedAt;
                await setTimeout(Math.max(3_100, Math.ceil(linkStageUpperBoundMs * 110)));
              },
            },
          ],
        },
        {},
        false,
        true,
      );
      expect(bundlerOptions.inputOptions.checks).toEqual(checks);
      expect(typeof bundlerOptions.inputOptions.pluginTimings === 'function').toBe(enabled);

      const getTimings = vi.fn(() => ({
        busyMs: 4_000,
        rows: [
          {
            owner: 'timing-check',
            kind: 'plugin' as const,
            hook: 'renderStart',
            calls: 1,
            ms: 4_000,
            maxInFlight: 1,
            overlapMs: 0,
            rankable: true,
          },
        ],
      }));
      bundlerOptions.inputOptions.pluginTimings = getTimings;

      const bundler = new BindingBundler();
      startAsyncRuntime();
      try {
        unwrapBindingResult(await bundler.generate(bundlerOptions));
      } finally {
        try {
          await bundler.close();
        } finally {
          shutdownAsyncRuntime();
        }
      }

      expect(getTimings).toHaveBeenCalledOnce();
      expect(onLog).toHaveBeenCalledTimes(enabled ? 1 : 0);
      if (enabled) {
        expect(onLog).toHaveBeenCalledWith(
          'warn',
          expect.objectContaining({ code: 'PLUGIN_TIMINGS' }),
          expect.any(Function),
        );
      }
    },
    60_000,
  );

  it.each(['bundlerTimings', 'pluginTimings'] as const)(
    'does not enable %s when measurement is disabled',
    async (option) => {
      const { bundlerOptions } = await createBundlerOptions(
        { checks: { [option]: true } },
        {},
        false,
        false,
      );
      expect(bundlerOptions.inputOptions.pluginTimings).toBeUndefined();
    },
  );

  it.each(['bundlerTimings', 'pluginTimings'] as const)(
    'rejects non-boolean values for %s',
    (option) => {
      expect(validateCliOptions({ checks: { [option]: 'true' } })[1]).toEqual([
        expect.stringContaining(`checks ${option}`),
      ]);
    },
  );
});
