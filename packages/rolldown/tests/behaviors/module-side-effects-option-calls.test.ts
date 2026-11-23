import type { Plugin } from 'rolldown';
import { rolldown } from 'rolldown';
import { expect, test } from 'vitest';

const modules: Record<string, string> = {
  entry: 'import "a"; import "b"; import "c"; import "ext"; console.log("ENTRY");',
  a: 'console.log("A");',
  b: 'console.log("B");',
  c: 'console.log("C");',
};

/**
 * Every module gets its `moduleSideEffects` from a hook: `resolveId` for most of them, `transform`
 * for `c`. `moduleParsed` reads the value of each module.
 */
function decidingPlugin(seen: Record<string, unknown[]>, extra: Partial<Plugin> = {}): Plugin {
  return {
    name: 'deciding',
    resolveId(id) {
      if (id === 'ext') return { id, external: true, moduleSideEffects: false };
      if (id === 'c') return id;
      if (id in modules) return { id, moduleSideEffects: true };
    },
    load: (id) => modules[id],
    transform(code, id) {
      if (id === 'c') return { code, moduleSideEffects: true };
    },
    moduleParsed(info) {
      (seen[info.id] ??= []).push(info.moduleSideEffects);
    },
    ...extra,
  };
}

async function build(plugin: Plugin, calls: string[]): Promise<string> {
  const bundle = await rolldown({
    input: 'entry',
    treeshake: {
      moduleSideEffects(id) {
        calls.push(id);
        return id !== 'a';
      },
    },
    plugins: [plugin],
  });
  try {
    const { output } = await bundle.generate();
    return output[0].code;
  } finally {
    await bundle.close();
  }
}

test('a treeshake.moduleSideEffects function does not run for a module whose value a hook decided', async () => {
  const calls: string[] = [];
  const seen: Record<string, unknown[]> = {};
  const code = await build(decidingPlugin(seen), calls);
  expect(calls).toEqual([]);
  expect(seen).toEqual({ entry: [true], a: [true], b: [true], c: [true] });
  expect(code).toContain('"A"');
});

test('a null write runs the treeshake.moduleSideEffects function once, for that module only', async () => {
  const calls: string[] = [];
  const seen: Record<string, unknown[]> = {};
  const plugin = decidingPlugin(seen);
  const code = await build(
    {
      ...plugin,
      moduleParsed(info) {
        if (info.id === 'a') {
          info.moduleSideEffects = null;
          (seen.afterWrite ??= []).push(info.moduleSideEffects, info.moduleSideEffects);
        }
      },
      buildEnd() {
        (seen.buildEnd ??= []).push(this.getModuleInfo('a')!.moduleSideEffects);
      },
    },
    calls,
  );
  expect(calls).toEqual(['a']);
  // The function gives `false` for `a`, and tree shaking uses it.
  expect(seen).toEqual({ afterWrite: [false, false], buildEnd: [false] });
  expect(code).not.toContain('"A"');
  expect(code).toContain('"B"');
});

test('a module that no hook decided runs the treeshake.moduleSideEffects function once', async () => {
  const calls: string[] = [];
  const seen: Record<string, unknown[]> = {};
  const plugin = decidingPlugin(seen);
  const code = await build(
    {
      ...plugin,
      resolveId(id, importer, options) {
        if (id === 'a') return id;
        return (plugin.resolveId as Function).call(this, id, importer, options);
      },
      load(id) {
        // Read before any hook decided, and again in `moduleParsed`.
        if (id === 'a') (seen.load ??= []).push(this.getModuleInfo(id)!.moduleSideEffects);
        return modules[id];
      },
    },
    calls,
  );
  expect(calls).toEqual(['a']);
  expect(seen).toEqual({ load: [false], entry: [true], a: [false], b: [true], c: [true] });
  expect(code).not.toContain('"A"');
});
