import { rolldown } from 'rolldown';
import { dev, getRuntimeCapabilities, getRuntimeSupport } from 'rolldown/experimental';
import { expect, test } from 'vitest';

const capabilities = getRuntimeCapabilities();
const expectThreadedWasi = process.env.ROLLDOWN_EXPECT_WASI_THREADS === '1';

test.runIf(capabilities.target === 'wasi-threads' || expectThreadedWasi)(
  'executes threaded WASI while preserving concurrent runtime leases',
  { timeout: 20_000 },
  async () => {
    expect(capabilities).toMatchObject({
      backend: 'tokio',
      flavor: 'MultiThread',
      target: 'wasi-threads',
      wasi: true,
      asyncRuntimeBuild: false,
      threads: true,
      watchSupported: false,
    });
    const support = getRuntimeSupport();
    expect(support.pluginErrorMetadata).toBe(true);
    expect(support.threadlessWasi).toBe(false);
    expect(support.workerd).toBe(false);

    let releaseLoad!: () => void;
    const loadGate = new Promise<void>((resolve) => {
      releaseLoad = resolve;
    });
    let loadStarted!: () => void;
    const loadStartedPromise = new Promise<void>((resolve) => {
      loadStarted = resolve;
    });
    const virtualPlugin = (blocked: boolean) => ({
      name: blocked ? 'blocked-virtual' : 'virtual',
      resolveId(id: string) {
        if (id === 'entry') return '\0entry';
      },
      async load(id: string) {
        if (id !== '\0entry') return;
        if (blocked) {
          loadStarted();
          await loadGate;
        }
        return 'export const value = 1';
      },
    });

    const first = await rolldown({
      input: 'entry',
      plugins: [virtualPlugin(false)],
    });
    const second = await rolldown({
      input: 'entry',
      plugins: [virtualPlugin(true)],
    });

    try {
      const firstOutput = await first.generate();
      expect(firstOutput.output).toHaveLength(1);
      const secondGenerate = second.generate();
      await loadStartedPromise;
      await first.close();
      releaseLoad();
      await expect(secondGenerate).resolves.toMatchObject({
        output: expect.arrayContaining([expect.objectContaining({ type: 'chunk' })]),
      });
    } finally {
      releaseLoad();
      await first.close();
      await second.close();
    }
  },
);

test.runIf(capabilities.target === 'wasi-threads' || expectThreadedWasi)(
  'preserves structured plugin errors across the threaded worker boundary',
  async () => {
    const cause = Object.assign(new RangeError('threaded nested cause'), {
      nestedMarker: 23,
    });
    const original = Object.assign(new TypeError('threaded plugin metadata failure'), {
      cause,
      code: 'THREADED_USER_CODE',
      customMarker: 'threaded-retained',
    });
    const bundle = await rolldown({
      input: 'entry',
      plugins: [
        {
          name: 'threaded-runtime-metadata-probe',
          resolveId(id) {
            if (id === 'entry') return '\0entry';
          },
          load(id) {
            if (id === '\0entry') return 'export default 1';
          },
          transform(_code, id) {
            if (id === '\0entry') throw original;
          },
        },
      ],
    });

    try {
      const failure = await bundle.generate().catch((error: unknown) => error);
      const [pluginError] = (failure as { errors?: unknown[] }).errors ?? [];
      expect(pluginError).toBe(original);
      expect(pluginError).toMatchObject({
        code: 'PLUGIN_ERROR',
        pluginCode: 'THREADED_USER_CODE',
        plugin: 'threaded-runtime-metadata-probe',
        hook: 'transform',
        id: '\0entry',
        customMarker: 'threaded-retained',
      });
      expect(original.stack).toContain('threaded plugin metadata failure');
      expect(original.cause).toBe(cause);
      expect(original.cause).toMatchObject({
        name: 'RangeError',
        message: 'threaded nested cause',
        nestedMarker: 23,
      });
    } finally {
      await bundle.close();
    }
  },
);

test.runIf(capabilities.target === 'wasi-threads' || expectThreadedWasi)(
  'runs and closes a threaded WASI dev engine',
  { timeout: 20_000 },
  async () => {
    expect(getRuntimeSupport().dev).toBe(true);

    let closeBundleCalls = 0;
    let outputCalls = 0;
    const engine = await dev(
      {
        input: 'entry',
        experimental: { devMode: true },
        plugins: [
          {
            name: 'threaded-wasi-dev-lifecycle',
            resolveId(id) {
              if (id === 'entry') return '\0entry';
            },
            load(id) {
              if (id === '\0entry') return 'export const value = 1';
            },
            closeBundle() {
              closeBundleCalls += 1;
            },
          },
        ],
      },
      {},
      {
        onOutput(result) {
          if (result instanceof Error) throw result;
          expect(result.output).toEqual([
            expect.objectContaining({
              type: 'chunk',
              exports: ['value'],
            }),
          ]);
          outputCalls += 1;
        },
      },
    );

    try {
      await engine.run();
      expect(outputCalls).toBe(1);
    } finally {
      await Promise.all([engine.close(), engine.close()]);
    }

    expect(closeBundleCalls).toBe(1);
    await expect(engine.ensureCurrentBuildFinish()).resolves.toBeUndefined();
    await expect(engine.run()).rejects.toThrow('Dev engine is closed');
    await expect(engine.close()).resolves.toBeUndefined();
  },
);
