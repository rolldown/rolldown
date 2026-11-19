import { setTimeout } from 'node:timers/promises';
import { expect, it, vi } from 'vitest';
import { BindingBundler, shutdownAsyncRuntime, startAsyncRuntime } from '../src/binding.cjs';
import { createBundlerOptions } from '../src/utils/create-bundler-option';
import { unwrapBindingResult } from '../src/utils/error';

vi.mock('../src/binding.cjs', async () => {
  const { loadBinding } = await import('./src/load-binding');
  return loadBinding();
});

it('counts a build that is still running when the next build starts', async () => {
  const bundler = new BindingBundler();
  const onLog = vi.fn();
  const getTimings = vi.fn(() => ({
    busyMs: 4_000,
    rows: [
      {
        owner: 'timing-check',
        kind: 'plugin' as const,
        hook: 'renderStart',
        calls: 1,
        ms: 4_000,
        maxMs: 4_000,
        maxInFlight: 1,
        overlapMs: 0,
        rankable: true,
      },
    ],
  }));

  // `buildEnd` runs before the link stage and `renderStart` after it, so `renderStart` gets an
  // upper bound of the link stage.
  async function bundlerOptionsFor(
    onRenderStart: (linkStageUpperBoundMs: number) => Promise<void>,
  ) {
    let buildEndAt = 0;
    const bundlerOptions = await createBundlerOptions(
      {
        input: 'entry',
        onLog,
        plugins: [
          {
            name: 'timing-check',
            resolveId: () => '\0entry',
            load: () => 'export default 1',
            buildEnd() {
              buildEndAt = performance.now();
            },
            async renderStart() {
              await onRenderStart(performance.now() - buildEndAt);
            },
          },
        ],
      },
      {},
      false,
      true,
    );
    bundlerOptions.inputOptions.pluginTimings = getTimings;
    return bundlerOptions;
  }

  let laterLinkStageUpperBoundMs = 0;
  const later = await bundlerOptionsFor(async (linkStageUpperBoundMs) => {
    laterLinkStageUpperBoundMs = linkStageUpperBoundMs;
  });
  const earlier = await bundlerOptionsFor(async (linkStageUpperBoundMs) => {
    // The later build starts after the link stage of this build, and it ends first.
    unwrapBindingResult(await bundler.generate(later));
    // The later build alone is too short for a report. The report comes only if the clocks of
    // this build count too.
    const linkStagesMs = linkStageUpperBoundMs + laterLinkStageUpperBoundMs;
    await setTimeout(Math.max(3_100, Math.ceil(linkStagesMs * 110)));
  });

  startAsyncRuntime();
  try {
    unwrapBindingResult(await bundler.generate(earlier));
  } finally {
    try {
      await bundler.close();
    } finally {
      shutdownAsyncRuntime();
    }
  }

  expect(getTimings).toHaveBeenCalledOnce();
  expect(onLog).toHaveBeenCalledOnce();
  expect(onLog).toHaveBeenCalledWith(
    'warn',
    expect.objectContaining({ code: 'PLUGIN_TIMINGS' }),
    expect.any(Function),
  );
}, 60_000);
