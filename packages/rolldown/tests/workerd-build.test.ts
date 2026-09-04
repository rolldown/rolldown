import { existsSync, readFileSync } from 'node:fs';
import { readFile } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';
import { afterEach, describe, expect, test, vi } from 'vitest';
// @ts-ignore This focused unit test intentionally reaches the package source outside the test rootDir.
import * as bindingProxy from '../src/binding-workerd-proxy';
// @ts-ignore This focused unit test intentionally reaches the package source outside the test rootDir.
import { RolldownMagicString as stubRolldownMagicString } from '../src/workerd-stubs/binding-magic-string';
// @ts-ignore This focused unit test intentionally reaches the package source outside the test rootDir.
import * as timerHostStub from '../src/workerd-stubs/timer-host';
// @ts-ignore Type-only view of the workerd entry; the dist bundle is imported at runtime.
import type * as workerdEntryTypes from '../src/workerd';

const wasip1LoaderPath = fileURLToPath(
  new URL('../src/rolldown-binding.wasip1.cjs', import.meta.url),
);
const distDir = new URL('../../browser/dist/', import.meta.url);
const distWorkerdPath = fileURLToPath(new URL('workerd.mjs', distDir));
const distWasmPath = fileURLToPath(new URL('rolldown-binding.wasm32-wasip1.wasm', distDir));

const PRIVATE_MANAGED_HOST_EXPORTS = [
  'getCurrentThreadTaskHostContractVersion',
  'isCurrentThreadHostRegistrationActive',
  'registerCurrentThreadTaskHost',
  'registerTimerHost',
  'reserveCurrentThreadHostRegistration',
  'unregisterCurrentThreadTaskHost',
  'unregisterTimerHost',
] as const;

const PROXY_ONLY_EXPORTS = [
  '__enterWorkerdBinding',
  '__exitWorkerdBinding',
  '__isWorkerdBindingProxy',
  // The generated cjs loader exports this next to the artifact exports; the
  // metadata header does not list it.
  '__rolldownBindingTarget',
] as const;

function readArtifactMetadataExports(): string[] {
  const source = readFileSync(wasip1LoaderPath, 'utf8');
  const match = source.match(/napi-rs-artifact-metadata:(\{.*\})/);
  expect(match, 'wasip1 loader must embed napi-rs-artifact-metadata').toBeTruthy();
  const metadata = JSON.parse(match![1]) as { exports: string[] };
  expect(Array.isArray(metadata.exports)).toBe(true);
  return metadata.exports;
}

describe('binding-workerd-proxy export surface', () => {
  test('module evaluation is side-effect free and matches the artifact metadata', () => {
    const artifactExports = readArtifactMetadataExports();
    const expected = new Set<string>(artifactExports);
    for (const privateExport of PRIVATE_MANAGED_HOST_EXPORTS) {
      expect(expected.delete(privateExport), `metadata should list ${privateExport}`).toBe(true);
    }
    for (const proxyOnly of PROXY_ONLY_EXPORTS) {
      expected.add(proxyOnly);
    }
    expect(new Set(Object.keys(bindingProxy))).toStrictEqual(expected);
  });

  test('reports the static threadless capability contract while inactive', () => {
    const capabilities = bindingProxy.getRuntimeCapabilities();
    expect(capabilities).toMatchObject({
      asyncRuntimeBuild: true,
      backend: 'shared',
      blockOnJsThreadSafe: false,
      devSupported: false,
      flavor: 'CurrentThread',
      target: 'wasi',
      threads: false,
      timers: false,
      wasi: true,
      watchSupported: false,
    });
    // The invariants normalizeRuntimeCapabilities() fails closed on.
    expect(capabilities.asyncRuntimeBuild).toBe(capabilities.backend === 'shared');
    expect(capabilities.threads).toBe(capabilities.flavor === 'MultiThread');
    expect(capabilities.wasi).toBe(capabilities.target !== 'native');
    expect(bindingProxy.__rolldownBindingTarget).toBe(capabilities.target);
  });

  test('forwards calls to the active exports and fails closed when inactive', () => {
    const parseSyncLoose = bindingProxy.parseSync as unknown as (...args: unknown[]) => unknown;
    expect(() => parseSyncLoose('a.js', 'let x')).toThrowError(
      /'parseSync' was used outside an active build/,
    );

    class FakeBundler {
      static tag = 'fake-bundler';
      args: unknown[];
      constructor(...args: unknown[]) {
        this.args = args;
      }
    }
    const fakeExports = {
      parseSync: (...args: unknown[]) => ['parsed', ...args],
      BindingBundler: FakeBundler,
      getRuntimeCapabilities: () => ({ delegated: true }),
    };

    bindingProxy.__enterWorkerdBinding(fakeExports);
    try {
      expect(parseSyncLoose('a.js', 'let x')).toStrictEqual(['parsed', 'a.js', 'let x']);
      const bundler = new bindingProxy.BindingBundler();
      expect(bundler).toBeInstanceOf(FakeBundler);
      expect((bindingProxy.BindingBundler as unknown as { tag: string }).tag).toBe('fake-bundler');
      expect(bindingProxy.getRuntimeCapabilities()).toStrictEqual({ delegated: true });
    } finally {
      bindingProxy.__exitWorkerdBinding(fakeExports);
    }

    expect(() => new bindingProxy.BindingBundler()).toThrowError(
      /'BindingBundler' was used outside an active build/,
    );
    expect(bindingProxy.getRuntimeCapabilities().target).toBe('wasi');
  });

  test('reference-counts one instance and rejects a second concurrent instance', () => {
    const first = { parseSync: () => 'first' };
    const second = { parseSync: () => 'second' };

    bindingProxy.__enterWorkerdBinding(first);
    try {
      bindingProxy.__enterWorkerdBinding(first);
      try {
        expect(() => bindingProxy.__enterWorkerdBinding(second)).toThrowError(
          /Another workerd Rolldown instance is currently active/,
        );
      } finally {
        bindingProxy.__exitWorkerdBinding(first);
      }
      // Still active: one reference remains.
      expect((bindingProxy.parseSync as unknown as () => unknown)()).toBe('first');
    } finally {
      bindingProxy.__exitWorkerdBinding(first);
    }
    expect(() => (bindingProxy.parseSync as unknown as () => unknown)()).toThrowError(
      /outside an active build/,
    );
    // Releasing a non-active exports object is a safe no-op.
    bindingProxy.__exitWorkerdBinding(second);
  });

  test('caches enum objects from the first active instance', () => {
    const enumValue = { Error: 0, Warn: 1 };
    const fakeExports = { BindingLogLevel: enumValue };
    const logLevel = bindingProxy.BindingLogLevel as unknown as Record<string, number>;
    expect(() => logLevel.Error).toThrowError(/outside an active build/);
    bindingProxy.__enterWorkerdBinding(fakeExports);
    try {
      expect(logLevel.Warn).toBe(1);
    } finally {
      bindingProxy.__exitWorkerdBinding(fakeExports);
    }
    // Enum objects are artifact constants: the cache outlives the instance.
    expect(logLevel.Error).toBe(0);
  });
});

describe('workerd stubs', () => {
  test('binding-magic-string stub is instanceof-safe and rejects construction', () => {
    const stubConstructor = stubRolldownMagicString as unknown as new () => unknown;
    expect(({ code: 'x' } as unknown as object) instanceof stubConstructor).toBe(false);
    // oxlint-disable-next-line no-explicit-any -- instanceof with a nullish left side is the case under test.
    expect((null as any) instanceof stubConstructor).toBe(false);
    expect(() => new stubConstructor()).toThrowError(
      /MagicString is not supported in the workerd build yet/,
    );
  });

  // The workerd bundle aliases `src/timer-host.ts` to this module by path, so
  // nothing imports it in a type-checked position: this is the only gate that
  // the stub stays export-free and registers no process-wide host on import.
  test('timer-host stub has no exports and no registration side effects', () => {
    expect(Object.keys(timerHostStub)).toStrictEqual([]);
  });
});

describe('workerd build() source entry', () => {
  test('rejects outside a bundled workerd context with a clear error', async () => {
    // @ts-ignore This focused unit test intentionally reaches the package source outside the test rootDir.
    const { build } = await import('../src/workerd-build');
    const fakeInstance = { dispose() {}, exports: {} };
    await expect(build({ instance: fakeInstance as never, input: 'x' })).rejects.toThrowError(
      /only functional from the bundled @rolldown\/browser\/workerd entry/,
    );
    await expect(build({ input: 'x' } as never)).rejects.toThrowError(
      /exactly one of `instance` or `module`/,
    );
    await expect(build({ instance: fakeInstance, module: {} } as never)).rejects.toThrowError(
      /exactly one of `instance` or `module`/,
    );
  }, 60_000);
});

// Unit tests for the `module:` path's private-instance ownership: the deferred
// loader's dispose() keeps a failed handle undisposed for a retry, and build()
// is that handle's only owner, so a failed dispose must park the instance for
// the next entry point instead of dropping the reference. Everything below the
// wrapper is mocked (scoped via doMock so the other describes keep the real
// modules), so these tests run purely against the source entry.
describe('workerd build() owned-instance disposal parking', () => {
  interface FakeOwnedInstance {
    exports: object;
    dispose: ReturnType<typeof vi.fn>;
  }

  function fakeOwnedInstance(dispose: () => void): FakeOwnedInstance {
    return { exports: {}, dispose: vi.fn(dispose) };
  }

  /** Throws `error` for the first `failures` calls, then succeeds. */
  function disposeFailingTimes(failures: number, error: Error): () => void {
    let calls = 0;
    return () => {
      calls += 1;
      if (calls <= failures) throw error;
    };
  }

  afterEach(() => {
    vi.doUnmock('../src/binding.cjs');
    vi.doUnmock('../src/rolldown-binding.wasip1-deferred.js');
    vi.doUnmock('../src/api/rolldown');
    vi.resetModules();
  });

  /**
   * Fresh copy of the source entry with the workerd-bundle marker set, its
   * private `createInstance` consuming `instanceQueue`, and the pipeline
   * stubbed: each `rolldown()` call consumes one `buildQueue` step ('ok'
   * generates a one-chunk result, an Error rejects the generate pass).
   */
  async function loadHarness(
    instanceQueue: FakeOwnedInstance[],
    buildQueue: Array<'ok' | Error>,
  ): Promise<Pick<typeof workerdEntryTypes, 'build' | 'createWorkerdBundle'>> {
    vi.resetModules();
    vi.doMock('../src/binding.cjs', () => ({ __isWorkerdBindingProxy: true }));
    vi.doMock('../src/rolldown-binding.wasip1-deferred.js', () => ({
      createInstance: vi.fn(async () => {
        const next = instanceQueue.shift();
        if (next === undefined) throw new Error('harness: no fake instance queued');
        return next;
      }),
    }));
    vi.doMock('../src/api/rolldown', () => ({
      rolldown: vi.fn(async () => {
        const step = buildQueue.shift() ?? 'ok';
        return {
          generate: async () => {
            if (step !== 'ok') throw step;
            return { output: [{ type: 'chunk' }] };
          },
          close: async () => {},
          __nativeCloseSettled: true,
          __whenNativeCloseSettled: async () => {},
        };
      }),
    }));
    // @ts-ignore This focused unit test intentionally reaches the package source outside the test rootDir.
    return await import('../src/workerd-build');
  }

  const wasmModule = {} as WebAssembly.Module;

  test('build-error branch: rejection surface unchanged, parked instance drained by a later build', async () => {
    const buildError = new Error('generate failed');
    const disposeError = new Error('transient host cleanup failure');
    // Fails the immediate attempt AND the one re-attempt, then recovers.
    const parked = fakeOwnedInstance(disposeFailingTimes(2, disposeError));
    const second = fakeOwnedInstance(() => {});
    const third = fakeOwnedInstance(() => {});
    const { build } = await loadHarness([parked, second, third], [buildError, 'ok', 'ok']);

    const rejection: unknown = await build({ module: wasmModule, input: 'x' }).catch((e) => e);
    expect(rejection).toBeInstanceOf(AggregateError);
    const aggregate = rejection as AggregateError;
    expect(aggregate.message).toBe('Build and workerd instance disposal both failed');
    expect(aggregate.errors).toStrictEqual([buildError, disposeError]);
    expect(aggregate.cause).toBe(buildError);
    // The immediate re-attempt ran before parking.
    expect(parked.dispose).toHaveBeenCalledTimes(2);

    // The next build() drains the parked instance before its own work...
    const result = await build({ module: wasmModule, input: 'x' });
    expect(result.output).toHaveLength(1);
    expect(parked.dispose).toHaveBeenCalledTimes(3);
    expect(second.dispose).toHaveBeenCalledTimes(1);

    // ...and a successful drain removes it: no further retries.
    await build({ module: wasmModule, input: 'x' });
    expect(parked.dispose).toHaveBeenCalledTimes(3);
  });

  test('success branch: raw dispose error surface unchanged, still-failing instance stays parked without failing later builds', async () => {
    const disposeError = new Error('persistent cleanup failure');
    const stuck = fakeOwnedInstance(() => {
      throw disposeError;
    });
    const second = fakeOwnedInstance(() => {});
    const third = fakeOwnedInstance(() => {});
    const { build } = await loadHarness([stuck, second, third], ['ok', 'ok', 'ok']);

    // The build itself succeeded; the raw dispose error still rejects, as today.
    const rejection: unknown = await build({ module: wasmModule, input: 'x' }).catch((e) => e);
    expect(rejection).toBe(disposeError);
    expect(stuck.dispose).toHaveBeenCalledTimes(2);

    // Later builds retry the parked instance but never fail because of it.
    const result = await build({ module: wasmModule, input: 'x' });
    expect(result.output).toHaveLength(1);
    expect(stuck.dispose).toHaveBeenCalledTimes(3);
    await build({ module: wasmModule, input: 'x' });
    expect(stuck.dispose).toHaveBeenCalledTimes(4);
  });

  test('a transient dispose failure recovers via the immediate re-attempt', async () => {
    const transient = fakeOwnedInstance(disposeFailingTimes(1, new Error('EBUSY once')));
    const { build } = await loadHarness([transient], ['ok']);
    const result = await build({ module: wasmModule, input: 'x' });
    expect(result.output).toHaveLength(1);
    expect(transient.dispose).toHaveBeenCalledTimes(2);
  });

  test('createWorkerdBundle() also drains parked instances', async () => {
    const parked = fakeOwnedInstance(disposeFailingTimes(2, new Error('cleanup failure')));
    const buildError = new Error('generate failed');
    const { build, createWorkerdBundle } = await loadHarness([parked], [buildError, 'ok']);

    await expect(build({ module: wasmModule, input: 'x' })).rejects.toThrowError(AggregateError);
    expect(parked.dispose).toHaveBeenCalledTimes(2);

    const callerOwned = fakeOwnedInstance(() => {});
    const bundle = await createWorkerdBundle(callerOwned as never, { input: 'x' });
    expect(parked.dispose).toHaveBeenCalledTimes(3);
    // The caller-owned instance is untouched by the drain.
    expect(callerOwned.dispose).not.toHaveBeenCalled();
    await bundle.close();
  });
});

// Full integration through the BUILT dist entry (node variant of the same
// bundle wiring the workerd condition uses): real wasm instance, real pipeline.
const distTest = test.runIf(existsSync(distWorkerdPath) && existsSync(distWasmPath));

interface VirtualGraph {
  files: Map<string, string>;
  plugin: (logSink?: string[]) => {
    name: string;
    resolveId: (id: string) => string | undefined;
    load: (id: string) => string | undefined;
  };
}

function makeVirtualGraph(moduleCount: number): VirtualGraph {
  const files = new Map<string, string>();
  files.set('virt:util.js', 'export function greet(name) { return `hello ${name}`; }\n');
  for (let i = 0; i < moduleCount; i++) {
    const next =
      i + 1 < moduleCount
        ? `import { value as next } from 'virt:mod-${i + 1}.js';`
        : 'const next = 1;';
    files.set(
      `virt:mod-${i}.js`,
      [
        next,
        "import { greet } from 'virt:util.js';",
        `export const value = ${i} + next;`,
        `export const label_${i} = greet('mod-${i}');`,
      ].join('\n'),
    );
  }
  files.set(
    'virt:entry.js',
    [
      "import { value, label_0 } from 'virt:mod-0.js';",
      'export const total = value;',
      'export const banner = label_0;',
    ].join('\n'),
  );
  return {
    files,
    plugin: () => ({
      name: 'virtual-graph',
      resolveId: (id: string) => (files.has(id) ? id : undefined),
      load: (id: string) => files.get(id),
    }),
  };
}

async function loadDistWorkerd(): Promise<{
  workerd: typeof workerdEntryTypes;
  wasmModule: WebAssembly.Module;
}> {
  const workerd = (await import(distWorkerdPath)) as typeof workerdEntryTypes;
  const wasmModule = await WebAssembly.compile(await readFile(distWasmPath));
  return { workerd, wasmModule };
}

describe('workerd build() against the built dist', () => {
  distTest(
    'builds a multi-module graph with rollup-style plugins on a caller-owned instance',
    async () => {
      const { workerd, wasmModule } = await loadDistWorkerd();
      const graph = makeVirtualGraph(20);
      const logs: string[] = [];
      const instance = await workerd.createInstance(wasmModule);
      try {
        const result = await workerd.build({
          instance,
          input: 'virt:entry.js',
          plugins: [
            {
              ...graph.plugin(),
              buildStart() {
                // Rollup-style context API must reach onLog.
                (this as { warn: (message: string) => void }).warn('buildStart ran');
              },
            },
          ],
          onLog: (level, log) => {
            logs.push(`${level}:${(log as { message: string }).message}`);
          },
          output: { format: 'esm' },
        });
        expect(result.output).toHaveLength(1);
        const chunk = result.output[0];
        expect(chunk.type).toBe('chunk');
        expect(chunk.fileName).toBe('virt_entry.js');
        if (chunk.type === 'chunk') {
          expect(chunk.code).toContain('hello ${name}');
          expect(chunk.code).toContain('total');
        }
        expect(logs).toContain('warn:buildStart ran');
      } finally {
        // Must succeed right away: the pipeline's terminal close releases the
        // bundler in the managed facade's open-object accounting.
        instance.dispose();
      }
      expect(instance.disposed).toBe(true);
    },
    180_000,
  );

  distTest(
    'createWorkerdBundle() generates repeatedly and excludes other instances until close',
    async () => {
      const { workerd, wasmModule } = await loadDistWorkerd();
      const graph = makeVirtualGraph(5);
      const instanceA = await workerd.createInstance(wasmModule);
      const instanceB = await workerd.createInstance(wasmModule);
      try {
        const bundle = await workerd.createWorkerdBundle(instanceA, {
          input: 'virt:entry.js',
          plugins: [graph.plugin()],
        });
        expect(bundle.closed).toBe(false);
        const esm = await bundle.generate({ format: 'esm' });
        const cjs = await bundle.generate({ format: 'cjs' });
        expect(esm.output[0].type).toBe('chunk');
        if (cjs.output[0].type === 'chunk') {
          expect(cjs.output[0].code).toContain('exports');
        }

        await expect(
          workerd.build({
            instance: instanceB,
            input: 'virt:entry.js',
            plugins: [graph.plugin()],
          }),
        ).rejects.toThrowError(/Another workerd Rolldown instance is currently active/);

        await bundle.close();
        expect(bundle.closed).toBe(true);

        // After close, the other instance can build.
        const result = await workerd.build({
          instance: instanceB,
          input: 'virt:entry.js',
          plugins: [graph.plugin()],
        });
        expect(result.output[0].fileName).toBe('virt_entry.js');
      } finally {
        instanceA.dispose();
        instanceB.dispose();
      }
    },
    180_000,
  );

  distTest(
    'a re-entrant close acknowledgement does not release the slot early',
    async () => {
      const { workerd, wasmModule } = await loadDistWorkerd();
      const graph = makeVirtualGraph(3);
      const instanceA = await workerd.createInstance(wasmModule);
      const instanceB = await workerd.createInstance(wasmModule);
      try {
        let bundle!: Awaited<ReturnType<typeof workerd.createWorkerdBundle>>;
        let inHook:
          | { innerResolved: boolean; closedInHook: boolean; admission: string }
          | undefined;
        bundle = await workerd.createWorkerdBundle(instanceA, {
          input: 'virt:entry.js',
          plugins: [
            graph.plugin(),
            {
              name: 'reentrant-close',
              closeBundle: async () => {
                // A close() from inside closeBundle is acknowledged early
                // while the REAL close is still running its cleanup; the
                // instance slot must not be released in that window.
                const innerResolved = await bundle.close().then(
                  () => true,
                  () => false,
                );
                const closedInHook = bundle.closed;
                const admission = await workerd
                  .build({
                    instance: instanceB,
                    input: 'virt:entry.js',
                    plugins: [graph.plugin()],
                  })
                  .then(
                    () => 'admitted',
                    (error: unknown) =>
                      error instanceof Error &&
                      /Another workerd Rolldown instance/.test(error.message)
                        ? 'refused'
                        : `unexpected: ${error}`,
                  );
                inHook = { innerResolved, closedInHook, admission };
              },
            },
          ],
        });
        await bundle.generate({ format: 'esm' });
        await bundle.close();
        expect(inHook).toBeDefined();
        expect(inHook!.innerResolved).toBe(true);
        expect(inHook!.closedInHook).toBe(false);
        expect(inHook!.admission).toBe('refused');
        expect(bundle.closed).toBe(true);
        // The slot released once the real close settled: dispose works and
        // the other instance can build.
        instanceA.dispose();
        const result = await workerd.build({
          instance: instanceB,
          input: 'virt:entry.js',
          plugins: [graph.plugin()],
        });
        expect(result.output[0].type).toBe('chunk');
      } finally {
        try {
          instanceA.dispose();
        } catch {
          // Disposed in the happy path above.
        }
        instanceB.dispose();
      }
    },
    180_000,
  );

  distTest(
    'a close() rejected while the bundle is open keeps the slot and allows retry',
    async () => {
      const { workerd, wasmModule } = await loadDistWorkerd();
      const graph = makeVirtualGraph(3);
      const instance = await workerd.createInstance(wasmModule);
      try {
        let bundle!: Awaited<ReturnType<typeof workerd.createWorkerdBundle>>;
        let hookCloseError: unknown;
        bundle = await workerd.createWorkerdBundle(instance, {
          input: 'virt:entry.js',
          plugins: [
            graph.plugin(),
            {
              name: 'close-from-hook',
              buildStart: async () => {
                // Closing from an active hook rejects upstream while the
                // native bundle stays open; the wrapper must not release the
                // slot or report the bundle closed.
                hookCloseError = await bundle.close().catch((error: unknown) => error);
              },
            },
          ],
        });
        const result = await bundle.generate({ format: 'esm' });
        expect(result.output[0].type).toBe('chunk');
        expect(hookCloseError).toBeInstanceOf(Error);
        expect(bundle.closed).toBe(false);
        // The instance slot is still held, so disposing must be refused...
        expect(() => instance.dispose()).toThrow();
        // ...until a retried close succeeds.
        await bundle.close();
        expect(bundle.closed).toBe(true);
      } finally {
        try {
          instance.dispose();
        } catch {
          // A failed assertion above may have left the bundle open; don't
          // let the refused dispose mask it.
        }
      }
    },
    180_000,
  );
});
