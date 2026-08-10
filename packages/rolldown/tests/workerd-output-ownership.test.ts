// Output-memory ownership on the threadless-WASI flavor, through the BUILT
// dist entry (the node variant of the same bundle the workerd condition wires
// up). That flavor snapshots every output field to JavaScript and releases the
// native payload immediately, because engines like workerd never run the GC
// finalizers that would otherwise free it.
import { existsSync } from 'node:fs';
import { readFile } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';
import { describe, expect, test } from 'vitest';
// @ts-ignore Type-only view of the workerd entry; the dist bundle is imported at runtime.
import type * as workerdEntryTypes from '../src/workerd';
// @ts-ignore Type-only view of the public output types.
import type { OutputChunk } from '../src/types/rolldown-output';

const distDir = new URL('../../browser/dist/', import.meta.url);
const distWorkerdPath = fileURLToPath(new URL('workerd.mjs', distDir));
const distWasmPath = fileURLToPath(new URL('rolldown-binding.wasm32-wasip1.wasm', distDir));

const distTest = test.runIf(existsSync(distWorkerdPath) && existsSync(distWasmPath));

function makeVirtualGraph(moduleCount: number): {
  files: Map<string, string>;
  plugin: () => {
    name: string;
    resolveId: (id: string) => string | undefined;
    load: (id: string) => string | undefined;
  };
} {
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

describe('workerd output ownership against the built dist', () => {
  distTest(
    'chunk.modules stays fully readable after build({ module }) disposed its private instance',
    async () => {
      const { workerd, wasmModule } = await loadDistWorkerd();
      const graph = makeVirtualGraph(10);
      const result = await workerd.build({
        module: wasmModule,
        input: 'virt:entry.js',
        plugins: [graph.plugin()],
        output: { format: 'esm' },
      });

      const chunk = result.output[0] as OutputChunk;
      expect(chunk.type).toBe('chunk');
      // The private instance is already disposed; every field of every module
      // must be JS-owned plain data, not a live call into the native side.
      const ids = Object.keys(chunk.modules);
      expect(ids.length).toBeGreaterThan(0);
      let renderedCode = '';
      for (const id of ids) {
        const rendered = chunk.modules[id];
        const code = rendered.code;
        expect(code === null || typeof code === 'string').toBe(true);
        renderedCode += code ?? '';
        expect(rendered.renderedLength).toBe(code?.length ?? 0);
        expect(Array.isArray(rendered.renderedExports)).toBe(true);
      }
      expect(renderedCode).toContain('hello ${name}');
      const utilModule = chunk.modules['virt:util.js'];
      expect(utilModule).toBeDefined();
      expect(utilModule.renderedExports).toContain('greet');
    },
    180_000,
  );

  distTest(
    'generateBundle hook copies are released per invocation while mutations apply and rebuilds work',
    async () => {
      const { workerd, wasmModule } = await loadDistWorkerd();
      const graph = makeVirtualGraph(5);
      const instance = await workerd.createInstance(wasmModule);
      try {
        const seenBundleKeys: string[][] = [];
        let stashedChunk: OutputChunk | undefined;
        const hook = {
          name: 'gb-probe',
          generateBundle(_options: unknown, bundle: Record<string, unknown>) {
            const keys = Object.keys(bundle);
            seenBundleKeys.push(keys);
            const chunk = bundle[keys[0]] as OutputChunk & { code: string };
            // Reading + writing `code` marks the chunk changed; the mutation
            // must survive the hook's bundle copy being released when this
            // invocation returns.
            chunk.code = `${chunk.code}\n// stamped-by-generate-bundle`;
            // Model a plugin stashing the bundle object past the hook.
            stashedChunk = chunk;
          },
        };

        const first = await workerd.build({
          instance,
          input: 'virt:entry.js',
          plugins: [graph.plugin(), hook],
          output: { format: 'esm' },
        });
        const firstChunk = first.output[0] as OutputChunk;
        expect(firstChunk.code).toContain('// stamped-by-generate-bundle');

        // The hook's copy was dropped after the invocation: fields read during
        // the round-trip stay readable (the mutable wrapper caches every read),
        // while `modules` -- which nothing read -- now throws on this flavor.
        expect(stashedChunk).toBeDefined();
        expect(stashedChunk!.code).toContain('// stamped-by-generate-bundle');
        expect(() => stashedChunk!.modules).toThrowError(/Memory has been freed/);

        // A rebuild on the same instance must invoke the hook with a fresh,
        // readable copy.
        const second = await workerd.build({
          instance,
          input: 'virt:entry.js',
          plugins: [graph.plugin(), hook],
          output: { format: 'esm' },
        });
        const secondChunk = second.output[0] as OutputChunk;
        expect(secondChunk.code).toContain('// stamped-by-generate-bundle');
        expect(seenBundleKeys).toHaveLength(2);
        expect(seenBundleKeys[1]).toEqual(seenBundleKeys[0]);
      } finally {
        instance.dispose();
      }
    },
    180_000,
  );

  distTest(
    're-emitting a returned binary asset copies its JavaScript bytes',
    async () => {
      const { workerd, wasmModule } = await loadDistWorkerd();
      const graph = makeVirtualGraph(3);
      const instance = await workerd.createInstance(wasmModule);
      const size = 64 * 1024;
      const makePattern = (seed: number) => {
        const bytes = new Uint8Array(size);
        for (let i = 0; i < size; i++) bytes[i] = (i * 7 + seed) & 0xff;
        return bytes;
      };
      const buildEmitting = async (source: Uint8Array) => {
        const result = await workerd.build({
          instance,
          input: 'virt:entry.js',
          plugins: [
            graph.plugin(),
            {
              name: 'emit-binary-asset',
              generateBundle() {
                this.emitFile({ type: 'asset', fileName: 'blob.bin', source });
              },
            },
          ],
        });
        const asset = result.output.find(
          (item) => item.type === 'asset' && item.fileName === 'blob.bin',
        );
        return (asset as { source: Uint8Array }).source;
      };

      try {
        const original = makePattern(0);
        const emitted = await buildEmitting(original);
        expect(Uint8Array.from(emitted)).toStrictEqual(original);

        // The returned array must carry its own JavaScript bytes: the native
        // payload is freed eagerly on this flavor, so a re-emit resolving
        // through the original native address would read freed (by now reused)
        // memory.
        const reemitted = await buildEmitting(emitted);
        expect(Uint8Array.from(reemitted)).toStrictEqual(original);

        for (let i = 1; i <= 2; i++) await buildEmitting(makePattern(i * 13));
        const afterChurn = await buildEmitting(emitted);
        expect(Uint8Array.from(afterChurn)).toStrictEqual(original);
      } finally {
        instance.dispose();
      }
    },
    180_000,
  );
});
