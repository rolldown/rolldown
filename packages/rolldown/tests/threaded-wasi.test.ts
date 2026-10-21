import { rolldown } from 'rolldown';
import { getRuntimeSupport } from 'rolldown/experimental';
import { expect, test } from 'vitest';

const support = getRuntimeSupport();
const expectThreadedWasi = process.env.ROLLDOWN_EXPECT_WASI_THREADS === '1';
// Threaded WASI is the WASI artifact that is not threadless.
const threadedWasi = (!support.parallelPlugins && !support.threadlessWasi) || expectThreadedWasi;

// The CurrentThread aliases of `resolve_runtime_flavor`
// (crates/rolldown_binding/src/async_runtime.rs); read once at binding load.
const laneIsSingle = ['single', 'single-thread', 'current', 'current-thread'].includes(
  process.env.ROLLDOWN_RUNTIME ?? '',
);

test.runIf(threadedWasi && !laneIsSingle)('runs MultiThread by default on threaded WASI', () => {
  // MultiThread here relies on napi's heap-sync workaround; see
  // internal-docs/async-runtime/implementation.md, "Threaded WASI heap sync".
  expect(support).toEqual({
    dev: true,
    watch: false,
    parallelPlugins: false,
    threadlessWasi: false,
    workerd: false,
  });
});

test.runIf(threadedWasi && laneIsSingle)(
  'runs CurrentThread on threaded WASI under ROLLDOWN_RUNTIME=single',
  () => {
    expect(support).toEqual({
      dev: false,
      watch: false,
      parallelPlugins: false,
      threadlessWasi: false,
      workerd: false,
    });
  },
);

test.runIf(threadedWasi)(
  'executes threaded WASI while overlapping builds survive a concurrent close',
  { timeout: 20_000 },
  async () => {
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

test.runIf(threadedWasi)(
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

test.runIf(threadedWasi)(
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
