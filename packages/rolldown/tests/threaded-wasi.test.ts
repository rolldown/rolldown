import { rolldown } from 'rolldown';
import { dev, getRuntimeCapabilities, getRuntimeSupport } from 'rolldown/experimental';
import { expect, test } from 'vitest';

const capabilities = getRuntimeCapabilities();
const expectThreadedWasi = process.env.ROLLDOWN_EXPECT_WASI_THREADS === '1';

test.runIf(capabilities.target === 'wasi-threads' || expectThreadedWasi)(
  'executes threaded WASI while preserving concurrent runtime leases',
  { timeout: 20_000 },
  async () => {
    // The threaded artifact runs the shared tokio-free scheduler like every
    // other artifact, and the resolver normalizes every non-native target to
    // CurrentThread (`crates/rolldown_binding/src/async_runtime.rs`): the
    // shared scheduler has no MultiThread executor on WebAssembly because
    // `napi-async-runtime` does not compile Rayon there. Real OS threads in
    // `wasm32-wasip1-threads` therefore change the loader, not the executor.
    expect(capabilities).toMatchObject({
      backend: 'shared',
      flavor: 'CurrentThread',
      target: 'wasi-threads',
      wasi: true,
      asyncRuntimeBuild: true,
      threads: false,
      devSupported: false,
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

// `dev()` needs a MultiThread executor to complete its initial build, and the
// threaded WASI artifact resolves to CurrentThread like every wasm artifact,
// so the binding reports `devSupported: false` and the public entry must fail
// closed before entering the binding instead of stalling on a build that can
// never finish.
test.runIf(capabilities.target === 'wasi-threads' || expectThreadedWasi)(
  'rejects threaded WASI dev engines before entering the binding',
  { timeout: 20_000 },
  async () => {
    expect(getRuntimeSupport().dev).toBe(false);

    let hookCalls = 0;
    await expect(
      dev(
        {
          input: 'entry',
          experimental: { devMode: true },
          plugins: [
            {
              name: 'threaded-wasi-dev-lifecycle',
              resolveId(id) {
                hookCalls += 1;
                if (id === 'entry') return '\0entry';
              },
              load(id) {
                hookCalls += 1;
                if (id === '\0entry') return 'export const value = 1';
              },
            },
          ],
        },
        {},
        {
          onOutput() {
            hookCalls += 1;
          },
        },
      ),
    ).rejects.toMatchObject({
      code: 'ERR_ROLLDOWN_UNSUPPORTED_RUNTIME_FEATURE',
      feature: 'dev',
    });
    expect(hookCalls).toBe(0);
  },
);
