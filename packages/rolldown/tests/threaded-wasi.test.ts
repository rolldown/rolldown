import { rolldown } from 'rolldown';
import {
  configureAsyncRuntime,
  getAsyncRuntimeConfig,
  getRuntimeCapabilities,
  getRuntimeSupport,
} from 'rolldown/experimental';
import { expect, test } from 'vitest';

const capabilities = getRuntimeCapabilities();
const expectThreadedWasi = process.env.ROLLDOWN_EXPECT_WASI_THREADS === '1';

// The lane's flavor, read once at binding load: MultiThread by default on the
// threaded artifact, CurrentThread under `ROLLDOWN_RUNTIME=single` (or an alias).
// See `resolve_runtime_config_for` in crates/rolldown_binding/src/async_runtime.rs.
const laneIsSingle = ['single', 'single-thread', 'current', 'current-thread'].includes(
  process.env.ROLLDOWN_RUNTIME ?? '',
);

function expectMultiThreadShape() {
  const config = getAsyncRuntimeConfig();
  expect(config.flavor).toBe('MultiThread');
  // 2 to 4 scheduler workers on threaded WASI.
  expect(config.workerThreads).toBeGreaterThanOrEqual(2);
  expect(config.workerThreads).toBeLessThanOrEqual(4);
  // Blocking admission keeps one runnable lane free.
  expect(config.maxBlockingTasks).toBeGreaterThanOrEqual(1);
  expect(config.maxBlockingTasks).toBeLessThanOrEqual(config.workerThreads - 1);
  // The capability report follows the configured flavor.
  expect(getRuntimeCapabilities()).toMatchObject({
    target: 'wasi-threads',
    wasi: true,
    flavor: 'MultiThread',
    threads: true,
    timers: true,
    devSupported: true,
    watchSupported: false,
  });
  // Parallel plugins and symlink traversal stay native-only on every WASI artifact.
  expect(getRuntimeSupport()).toMatchObject({
    dev: true,
    watch: false,
    parallelPlugins: false,
    symlinks: false,
    threadlessWasi: false,
    workerd: false,
  });
}

function expectCurrentThreadShape() {
  expect(getAsyncRuntimeConfig()).toMatchObject({
    flavor: 'CurrentThread',
    workerThreads: 1,
    maxBlockingTasks: 1,
  });
  expect(getRuntimeCapabilities()).toMatchObject({
    target: 'wasi-threads',
    wasi: true,
    flavor: 'CurrentThread',
    threads: false,
    timers: true,
    devSupported: false,
    watchSupported: false,
  });
  expect(getRuntimeSupport()).toMatchObject({
    dev: false,
    watch: false,
    parallelPlugins: false,
    symlinks: false,
    threadlessWasi: false,
    workerd: false,
  });
}

test.runIf((capabilities.target === 'wasi-threads' || expectThreadedWasi) && !laneIsSingle)(
  'runs MultiThread by default on threaded WASI',
  () => {
    // See internal-docs/wasi-shared-memory-grow/design.md for why this is safe.
    expectMultiThreadShape();
    if (process.env.ROLLDOWN_WORKER_THREADS === undefined) {
      expect(getAsyncRuntimeConfig()).toMatchObject({ workerThreads: 2, maxBlockingTasks: 1 });
    }
  },
);

test.runIf((capabilities.target === 'wasi-threads' || expectThreadedWasi) && laneIsSingle)(
  'runs CurrentThread on threaded WASI under ROLLDOWN_RUNTIME=single',
  () => {
    expectCurrentThreadShape();
  },
);

test.runIf(capabilities.target === 'wasi-threads' || expectThreadedWasi)(
  'accepts either flavor before first use on threaded WASI',
  () => {
    // MultiThread is rejected only on threadless WASI.
    const initial = getAsyncRuntimeConfig();
    try {
      configureAsyncRuntime({ flavor: 'CurrentThread' });
      expectCurrentThreadShape();
      // 2 from a CurrentThread start: the MultiThread minimum.
      configureAsyncRuntime({ flavor: 'MultiThread' });
      expectMultiThreadShape();
      expect(getAsyncRuntimeConfig().workerThreads).toBe(2);
    } finally {
      // Put back the lane's own configuration so the rest of this file runs on it.
      configureAsyncRuntime({
        flavor: initial.flavor,
        workerThreads: initial.workerThreads,
        maxBlockingTasks: initial.maxBlockingTasks,
      });
    }
    expect(getAsyncRuntimeConfig()).toEqual(initial);
    expect(getRuntimeCapabilities().flavor).toBe(initial.flavor);
  },
);

test.runIf(capabilities.target === 'wasi' && !expectThreadedWasi)(
  'rejects the MultiThread opt-in on threadless WASI',
  () => {
    // Rolldown's own guard: threadless wasm32-wasip1 has no threads to run workers on.
    expect(() => configureAsyncRuntime({ flavor: 'MultiThread' })).toThrow(
      'the multi-thread runtime is unavailable in this WebAssembly build',
    );
    // A rejected configure leaves the configuration untouched.
    expect(getAsyncRuntimeConfig().flavor).toBe('CurrentThread');
  },
);

test.runIf(capabilities.target === 'wasi-threads' || expectThreadedWasi)(
  'executes threaded WASI while overlapping builds survive a concurrent close',
  { timeout: 20_000 },
  async () => {
    // Runs on the lane's flavor: MultiThread by default, CurrentThread when the
    // lane sets `ROLLDOWN_RUNTIME=single` (`crates/rolldown_binding/src/async_runtime.rs`).
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
  'summarizes a nullish output-option rejection across the threaded worker boundary',
  async () => {
    const bundle = await rolldown({
      input: 'entry',
      plugins: [
        {
          name: 'threaded-nullish-rejection-probe',
          resolveId(id) {
            if (id === 'entry') return '\0entry';
          },
          load(id) {
            if (id === '\0entry') return 'export default 1';
          },
        },
      ],
    });

    try {
      const failure = await bundle
        .generate({
          entryFileNames: () => {
            throw undefined;
          },
        })
        .catch((error: unknown) => error);
      expect(failure).toBeInstanceOf(Error);
      expect((failure as Error).message).not.toContain(
        'Cannot convert undefined or null to object',
      );
      expect((failure as Error).message).toContain('Error: undefined');
      // the retained primitive still reaches the caller unchanged
      expect((failure as { errors?: unknown[] }).errors).toHaveLength(1);
      expect((failure as { errors: unknown[] }).errors[0]).toBe(undefined);
    } finally {
      await bundle.close();
    }
  },
);
