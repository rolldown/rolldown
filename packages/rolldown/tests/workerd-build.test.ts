import { existsSync } from 'node:fs';
import { readFile } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';
import { describe, expect, test, vi } from 'vitest';
import * as bindingProxy from '../src/binding-workerd-proxy';
import { RolldownMagicString as stubRolldownMagicString } from '../src/workerd-stubs/binding-magic-string';
import type * as workerdEntryTypes from '../src/workerd';

const distDir = new URL('../../browser/dist/', import.meta.url);
const distWorkerdPath = fileURLToPath(new URL('workerd.mjs', distDir));
const distWasmPath = fileURLToPath(new URL('rolldown-binding.wasm32-wasip1.wasm', distDir));

// `@napi-rs/cli` reports the loaded flavor's `platformArchABI`; the Rust
// `get_runtime_capabilities()` report spells the same artifact differently.
const CAPABILITY_TARGET_TO_BINDING_TARGET: Record<string, string> = {
  native: 'native',
  wasi: 'wasm32-wasip1',
  'wasi-threads': 'wasm32-wasi',
};

describe('binding-workerd-proxy', () => {
  test('reports the static threadless capability contract while inactive', () => {
    const capabilities = bindingProxy.getRuntimeCapabilities();
    expect(capabilities).toMatchObject({
      devSupported: false,
      flavor: 'CurrentThread',
      target: 'wasi',
      threads: false,
      wasi: true,
      watchSupported: false,
    });
    // The invariants the native `get_runtime_capabilities()` report holds.
    expect(capabilities.threads).toBe(capabilities.flavor === 'MultiThread');
    expect(capabilities.wasi).toBe(capabilities.target !== 'native');
    expect(bindingProxy.__napiBindingTarget).toBe(
      CAPABILITY_TARGET_TO_BINDING_TARGET[capabilities.target],
    );
    // `workerd-build.ts` detects a workerd bundle by this marker.
    expect(bindingProxy.__isWorkerdBindingProxy).toBe(true);
  });

  test('forwards calls to the active exports and fails closed when inactive', () => {
    const parseSyncLoose = bindingProxy.lazyExport('parseSync') as (...args: unknown[]) => unknown;
    const BindingBundler = bindingProxy.lazyExport('BindingBundler');
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
      const bundler = new BindingBundler();
      expect(bundler).toBeInstanceOf(FakeBundler);
      expect(BindingBundler.tag).toBe('fake-bundler');
      expect(bindingProxy.getRuntimeCapabilities()).toStrictEqual({ delegated: true });
    } finally {
      bindingProxy.__exitWorkerdBinding(fakeExports);
    }

    expect(() => new BindingBundler()).toThrowError(
      /'BindingBundler' was used outside an active build/,
    );
    expect(bindingProxy.getRuntimeCapabilities().target).toBe('wasi');
  });

  test('reference-counts one instance and rejects a second concurrent instance', () => {
    const first = { parseSync: () => 'first' };
    const second = { parseSync: () => 'second' };
    const parseSync = bindingProxy.lazyExport('parseSync') as () => unknown;

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
      expect(parseSync()).toBe('first');
    } finally {
      bindingProxy.__exitWorkerdBinding(first);
    }
    expect(() => parseSync()).toThrowError(/outside an active build/);
    // Releasing a non-active exports object is a safe no-op.
    bindingProxy.__exitWorkerdBinding(second);
  });

  test('supports instanceof and prototype reads while active', () => {
    class FakeBundler {}
    const fakeExports = { BindingBundler: FakeBundler };
    const BindingBundler = bindingProxy.lazyExport('BindingBundler');
    expect(BindingBundler.name).toBe('BindingBundler');
    expect(() => ({}) instanceof BindingBundler).toThrowError(/outside an active build/);
    bindingProxy.__enterWorkerdBinding(fakeExports);
    try {
      expect(BindingBundler.prototype).toBe(FakeBundler.prototype);
      expect(new BindingBundler() instanceof BindingBundler).toBe(true);
      expect(new FakeBundler() instanceof BindingBundler).toBe(true);
      expect({} instanceof BindingBundler).toBe(false);
    } finally {
      bindingProxy.__exitWorkerdBinding(fakeExports);
    }
  });

  test('caches enum objects from the first active instance', () => {
    const enumValue = { Error: 0, Warn: 1 };
    const fakeExports = { BindingLogLevel: enumValue };
    const logLevel = bindingProxy.lazyExport('BindingLogLevel') as Record<string, number>;
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
    expect((null as any) instanceof stubConstructor).toBe(false);
    expect(() => new stubConstructor()).toThrowError(
      /MagicString is not supported in the workerd build yet/,
    );
  });
});

describe('workerd build() source entry', () => {
  test('rejects outside a bundled workerd context with a clear error', async () => {
    const { build } = await import('../src/workerd-build');
    const fakeInstance = { dispose() {} };
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

describe('workerd build() private-instance disposal', () => {
  test('retries a failed private dispose, then reports one that keeps failing and releases it', async () => {
    let live = 0;
    let failDisposals = 0;
    let disposeCalls = 0;
    let buildError: Error | undefined;
    vi.resetModules();
    vi.doMock('../src/binding.cjs', () => ({ __isWorkerdBindingProxy: true }));
    vi.doMock('../src/api/build', () => ({
      build: async () => {
        if (buildError) throw buildError;
        return { output: [] };
      },
    }));
    // The loader's documented retryable failure: native cleanup still queued.
    vi.doMock('../src/rolldown-binding.wasip1-deferred.js', () => ({
      getDeferredRuntimeStats: () => ({ liveInstances: live }),
      createInstance: async () => {
        live += 1;
        let disposed = false;
        return {
          exports: {},
          memory: new WebAssembly.Memory({ initial: 1, maximum: 1 }),
          memoryBytes: 65_536,
          get disposed() {
            return disposed;
          },
          async dispose() {
            if (disposed) return;
            disposeCalls += 1;
            if (failDisposals > 0) {
              failDisposals -= 1;
              throw Object.assign(new Error('1 queued settlement(s); retry dispose()'), {
                code: 'ERR_NAPI_WASI_CLEANUP_PENDING',
              });
            }
            disposed = true;
            live -= 1;
          },
        };
      },
    }));
    try {
      const workerd = await import('../src/workerd');
      const module = {} as WebAssembly.Module;
      const liveInstances = () => workerd.getWorkerdRuntimeStats().liveInstances;

      // The first dispose rejects right after the build; the retry frees it.
      failDisposals = 1;
      await expect(workerd.build({ module, input: 'x' })).resolves.toEqual({ output: [] });
      expect(disposeCalls).toBe(2);
      expect(liveInstances()).toBe(0);

      // Build and every retry fail: the build error stays primary, with the
      // dispose error as its cause, after exactly three tries.
      failDisposals = Number.POSITIVE_INFINITY;
      buildError = new Error('build failed');
      disposeCalls = 0;
      const failure = await workerd.build({ module, input: 'x' }).catch((error: unknown) => error);
      expect(failure).toBe(buildError);
      expect((failure as Error).cause).toMatchObject({ code: 'ERR_NAPI_WASI_CLEANUP_PENDING' });
      expect(disposeCalls).toBe(3);
      // The mock counts an instance live until a dispose succeeds.
      expect(liveInstances()).toBe(1);

      // The handle was released, not kept: nothing retries its dispose.
      failDisposals = 0;
      buildError = undefined;
      const next = await workerd.createInstance(module);
      expect(disposeCalls).toBe(3);
      await expect(workerd.build({ instance: next, input: 'x' })).resolves.toEqual({ output: [] });
      expect(disposeCalls).toBe(3);
      expect(liveInstances()).toBe(2);

      // Entry is synchronous: a dispose() made right after build() is refused.
      const building = workerd.build({ instance: next, input: 'x' });
      const disposing = next.dispose();
      await expect(disposing).rejects.toThrow(/with 1 active binding operation/);
      await expect(building).resolves.toEqual({ output: [] });
      expect(next.disposed).toBe(false);
      await next.dispose();
      expect(liveInstances()).toBe(1);
    } finally {
      vi.doUnmock('../src/binding.cjs');
      vi.doUnmock('../src/api/build');
      vi.doUnmock('../src/rolldown-binding.wasip1-deferred.js');
      vi.resetModules();
    }
  });
});

// Through the built dist workerd entry: real wasm instance, real pipeline.
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
        // The result holds no binding wrapper of the instance.
        expect((chunk as unknown as { bindingChunk?: unknown }).bindingChunk).toBeUndefined();
        if (chunk.type === 'chunk') {
          expect(chunk.code).toContain('hello ${name}');
          expect(chunk.code).toContain('total');
        }
        expect(logs).toContain('warn:buildStart ran');
      } finally {
        // Must succeed right away: the settled build released the instance.
        await instance.dispose();
      }
      expect(instance.disposed).toBe(true);
    },
    180_000,
  );

  distTest(
    'concurrent builds share one instance, which no other instance or dispose() can take',
    async () => {
      const { workerd, wasmModule } = await loadDistWorkerd();
      const graph = makeVirtualGraph(5);
      const instanceA = await workerd.createInstance(wasmModule);
      const instanceB = await workerd.createInstance(wasmModule);
      let release!: () => void;
      const parked = new Promise<void>((resolve) => {
        release = resolve;
      });
      let entered!: () => void;
      const enteredLoad = new Promise<void>((resolve) => {
        entered = resolve;
      });
      const holdPlugin = {
        name: 'hold-load',
        load: async (id: string) => {
          if (id !== 'virt:util.js') return undefined;
          entered();
          await parked;
          return graph.files.get(id);
        },
      };
      try {
        const slow = workerd.build({
          instance: instanceA,
          input: 'virt:entry.js',
          plugins: [holdPlugin, graph.plugin()],
        });
        slow.catch(() => {});
        await enteredLoad;
        const fast = workerd.build({
          instance: instanceA,
          input: 'virt:entry.js',
          plugins: [graph.plugin()],
          output: { format: 'cjs' },
        });

        await expect(instanceA.dispose()).rejects.toThrow(
          'Cannot dispose this workerd Rolldown instance with 2 active binding operations; ' +
            'await active operations and close every binding object first',
        );
        await expect(
          workerd.build({ instance: instanceB, input: 'virt:entry.js', plugins: [graph.plugin()] }),
        ).rejects.toThrow(/Another workerd Rolldown instance is currently active/);

        const fastResult = await fast;
        if (fastResult.output[0].type === 'chunk') {
          expect(fastResult.output[0].code).toContain('exports');
        }
        release();
        expect((await slow).output[0].fileName).toBe('virt_entry.js');

        const other = await workerd.build({
          instance: instanceB,
          input: 'virt:entry.js',
          plugins: [graph.plugin()],
        });
        expect(other.output[0].type).toBe('chunk');
        await instanceA.dispose();
        expect(() => instanceA.memory).toThrow('This workerd Rolldown instance has been disposed');
        await expect(
          workerd.build({ instance: instanceA, input: 'virt:entry.js', plugins: [graph.plugin()] }),
        ).rejects.toThrow('This workerd Rolldown instance has been disposed');
      } finally {
        release();
        await instanceA.dispose().catch(() => {});
        await instanceB.dispose();
      }
    },
    180_000,
  );

  // closeBundle runs while the build's bundle is still open, so the instance
  // must stay held until build() settles, not until the hook returns.
  distTest(
    'a build holds its instance through closeBundle',
    async () => {
      const { workerd, wasmModule } = await loadDistWorkerd();
      const graph = makeVirtualGraph(3);
      const instanceA = await workerd.createInstance(wasmModule);
      const instanceB = await workerd.createInstance(wasmModule);
      const outcome = (work: Promise<unknown>) =>
        work.then(
          () => 'fulfilled',
          (error: unknown) => (error instanceof Error ? error.message : String(error)),
        );
      try {
        let inHook: { dispose: string; otherBuild: string } | undefined;
        await workerd.build({
          instance: instanceA,
          input: 'virt:entry.js',
          plugins: [
            graph.plugin(),
            {
              name: 'close-bundle-window',
              closeBundle: async () => {
                inHook = {
                  dispose: await outcome(instanceA.dispose()),
                  otherBuild: await outcome(
                    workerd.build({
                      instance: instanceB,
                      input: 'virt:entry.js',
                      plugins: [graph.plugin()],
                    }),
                  ),
                };
              },
            },
          ],
        });
        expect(inHook?.dispose).toMatch(
          /^Cannot dispose this workerd Rolldown instance with 1 active binding operation;/,
        );
        expect(inHook?.otherBuild).toMatch(/Another workerd Rolldown instance is currently active/);
        // Released once build() settled: dispose works and the other instance builds.
        await instanceA.dispose();
        const result = await workerd.build({
          instance: instanceB,
          input: 'virt:entry.js',
          plugins: [graph.plugin()],
        });
        expect(result.output[0].type).toBe('chunk');
      } finally {
        await instanceA.dispose().catch(() => {});
        await instanceB.dispose();
      }
    },
    180_000,
  );

  distTest(
    'build({ module }) disposes its private instance whether the build succeeds or fails',
    async () => {
      const { workerd, wasmModule } = await loadDistWorkerd();
      const graph = makeVirtualGraph(3);
      const before = workerd.getWorkerdRuntimeStats().liveInstances;
      const result = await workerd.build({
        module: wasmModule,
        input: 'virt:entry.js',
        plugins: [graph.plugin()],
      });
      expect(result.output[0].type).toBe('chunk');
      expect(workerd.getWorkerdRuntimeStats().liveInstances).toBe(before);

      await expect(
        workerd.build({ module: wasmModule, input: 'virt:missing.js', plugins: [graph.plugin()] }),
      ).rejects.toThrow(/virt:missing\.js/);
      expect(workerd.getWorkerdRuntimeStats().liveInstances).toBe(before);
    },
    180_000,
  );

  // The binding wrappers' methods close over their instance's emnapi env, so a
  // result that kept them would keep the disposed instance's Memory alive.
  distTest(
    'a retained build({ module }) result holds no binding wrapper and stays readable',
    async () => {
      const { workerd, wasmModule } = await loadDistWorkerd();
      const graph = makeVirtualGraph(3);
      const result = await workerd.build({
        module: wasmModule,
        input: 'virt:entry.js',
        plugins: [
          graph.plugin(),
          {
            name: 'emit-assets',
            generateBundle() {
              const context = this as { emitFile: (file: object) => string };
              context.emitFile({
                type: 'asset',
                fileName: 'bin.dat',
                source: new Uint8Array([1, 2, 3, 4]),
              });
              context.emitFile({ type: 'asset', fileName: 'note.txt', source: 'text asset' });
            },
          },
        ],
        output: { sourcemap: true },
      });

      expect((result as unknown as { bindingOutputs?: unknown }).bindingOutputs).toBeUndefined();
      const chunks = result.output.filter((item) => item.type === 'chunk');
      const assets = result.output.filter((item) => item.type === 'asset');
      expect(chunks).toHaveLength(1);
      expect(assets.map((asset) => asset.fileName)).toEqual(
        expect.arrayContaining(['bin.dat', 'note.txt']),
      );
      for (const item of result.output) {
        const fields = item as unknown as Record<string, unknown>;
        expect(item.type === 'chunk' ? fields.bindingChunk : fields.bindingAsset).toBeUndefined();
        // Every public field is cached and still readable.
        for (const key of Object.keys(item)) expect(() => fields[key]).not.toThrow();
      }
      expect(Object.keys(chunks[0])).toEqual(
        expect.arrayContaining([
          'fileName',
          'name',
          'exports',
          'isEntry',
          'facadeModuleId',
          'isDynamicEntry',
          'sourcemapFileName',
          'preliminaryFileName',
          'code',
          'modules',
          'imports',
          'dynamicImports',
          'moduleIds',
          'map',
        ]),
      );
      const [chunk] = chunks;
      if (chunk.type === 'chunk') {
        expect(chunk.code).toContain('hello ${name}');
        expect(chunk.map?.mappings).toBeTruthy();
        expect(Object.keys(chunk.modules)).toContain('virt:entry.js');
        expect(chunk.modules['virt:entry.js'].renderedLength).toBeGreaterThan(0);
      }
      const bin = assets.find((asset) => asset.fileName === 'bin.dat');
      expect(bin?.source).toEqual(new Uint8Array([1, 2, 3, 4]));
      expect(assets.find((asset) => asset.fileName === 'note.txt')?.source).toBe('text asset');

      // Reports a status instead of calling into the destroyed environment.
      const status = result.__rolldown_external_memory_handle__();
      expect(status.freed).toBe(false);
      expect(status.reason).toMatch(/Memory has already been freed/);
    },
    180_000,
  );
});
