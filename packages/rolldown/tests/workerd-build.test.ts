import { existsSync, readFileSync } from 'node:fs';
import { readFile } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';
import { describe, expect, test } from 'vitest';
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
    'builds with a caller-provided module and disposes its private instance',
    async () => {
      const { workerd, wasmModule } = await loadDistWorkerd();
      const graph = makeVirtualGraph(5);
      const before = workerd.getWorkerdRuntimeStats().liveInstances;
      const result = await workerd.build({
        module: wasmModule,
        input: 'virt:entry.js',
        plugins: [graph.plugin()],
      });
      expect(workerd.getWorkerdRuntimeStats().liveInstances).toBe(before);
      const chunk = result.output[0];
      // Outputs stay readable after the private instance was disposed:
      // the threadless flavor copies output payloads to JavaScript eagerly.
      if (chunk.type === 'chunk') {
        expect(chunk.code).toContain('hello ${name}');
      }
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
    'surfaces build failures as an ANSI-free BundleError and keeps the instance usable',
    async () => {
      const { workerd, wasmModule } = await loadDistWorkerd();
      const graph = makeVirtualGraph(3);
      const instance = await workerd.createInstance(wasmModule);
      try {
        let caught: unknown;
        try {
          await workerd.build({
            instance,
            input: 'virt:not-there.js',
            plugins: [graph.plugin()],
          });
        } catch (error) {
          caught = error;
        }
        expect(caught).toBeInstanceOf(Error);
        const bundleError = caught as Error & { errors?: Array<{ message?: string }> };
        expect(bundleError.message).toMatch(/Build failed with 1 error/);
        expect(bundleError.message).toMatch(/virt:not-there\.js/);
        // oxlint-disable-next-line no-control-regex -- asserting ANSI escapes were stripped.
        expect(bundleError.message).not.toMatch(/\u001B/);
        // `stack` is materialized at construction with the colored message and
        // must be stripped separately from `message`.
        // oxlint-disable-next-line no-control-regex -- asserting ANSI escapes were stripped.
        expect(bundleError.stack ?? '').not.toMatch(/\u001B/);
        expect(bundleError.errors).toHaveLength(1);

        // A failed build must not leak the active-binding slot.
        const result = await workerd.build({
          instance,
          input: 'virt:entry.js',
          plugins: [graph.plugin()],
        });
        expect(result.output[0].fileName).toBe('virt_entry.js');
      } finally {
        instance.dispose();
      }
    },
    180_000,
  );
});
