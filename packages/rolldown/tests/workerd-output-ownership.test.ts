// Output-memory ownership on the threadless-WASI flavor, through the BUILT
// dist entry (the node variant of the same bundle wiring the workerd
// condition uses). The threadless flavor snapshots every output field to
// JavaScript eagerly and releases the native payload immediately, because
// engines like workerd never run the GC finalizers that would otherwise free
// it. These tests pin the two ownership contracts added for that flavor:
//
// 1. `chunk.modules` values are plain JS-owned data — readable after the
//    build's private instance is disposed (previously they were live getters
//    into `BindingRenderedModule` boxes and threw after dispose).
// 2. Each generateBundle/writeBundle hook invocation's marshaled bundle copy
//    is released when the hook round-trip finishes: mutations still apply,
//    rebuilds on the same instance keep working, and a bundle object stashed
//    past the hook gets throwing getters for fields it never read.
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
            // must reach the final output even though the hook's bundle copy
            // is released right after this invocation returns.
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

        // The hook's marshaled copy was dropped after the invocation: fields
        // read during the hook round-trip stay readable (the mutable wrapper
        // caches every read, and collecting the changed chunk read most of
        // them), while `modules` — which nothing read — now throws on this
        // flavor.
        expect(stashedChunk).toBeDefined();
        expect(stashedChunk!.code).toContain('// stamped-by-generate-bundle');
        expect(() => stashedChunk!.modules).toThrowError(/Memory has been freed/);

        // A rebuild on the same instance keeps working and invokes the hook
        // with a fresh, readable copy.
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
});
