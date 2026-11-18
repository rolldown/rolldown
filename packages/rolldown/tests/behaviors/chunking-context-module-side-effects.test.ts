import type { ModuleInfo, PluginContext } from 'rolldown';
import { rolldown } from 'rolldown';
import { expect, test } from 'vitest';

const modules: Record<string, string> = {
  entry: 'import "dep"; import "ext"; console.log("entry");',
  dep: 'console.log("DEP");',
};

test('the chunking context reads and writes the same moduleSideEffects as the plugin context', async () => {
  let getPluginModuleInfo: PluginContext['getModuleInfo'];
  const seen: Record<string, unknown[]> = { dep: [], ext: [] };
  let renderChunkCalls = 0;
  const build = await rolldown({
    input: 'entry',
    external: ['ext'],
    plugins: [
      {
        name: 'virtual-modules',
        resolveId(id) {
          if (id === 'dep') return { id, moduleSideEffects: false };
          if (id !== 'ext') return id;
        },
        load: (id) => modules[id],
        buildEnd() {
          getPluginModuleInfo = this.getModuleInfo;
        },
        renderChunk() {
          renderChunkCalls++;
          // The writes of the chunking context below.
          expect(this.getModuleInfo('dep')!.moduleSideEffects).toBe(true);
          expect(this.getModuleInfo('ext')!.moduleSideEffects).toBe(false);
        },
      },
    ],
  });
  try {
    const { output } = await build.generate({
      codeSplitting: {
        groups: [
          {
            name(_id, ctx) {
              if (seen.dep.length > 0) return null;
              for (const id of ['dep', 'ext']) {
                const info: ModuleInfo = ctx.getModuleInfo(id)!;
                const values = seen[id];
                values.push(info.moduleSideEffects);
                // `null` means "not decided": no option and no `package.json`, so `true`.
                info.moduleSideEffects = null;
                values.push(info.moduleSideEffects, getPluginModuleInfo(id)!.moduleSideEffects);
                getPluginModuleInfo(id)!.moduleSideEffects = id === 'dep';
                values.push(info.moduleSideEffects);
              }
              ctx.getModuleInfo('ext')!.moduleSideEffects = false;
              return null;
            },
          },
        ],
      },
    });
    expect(seen).toStrictEqual({
      dep: [false, true, true, true],
      ext: [true, true, true, false],
    });
    expect(renderChunkCalls).toBeGreaterThan(0);
    // Tree shaking ran before these writes, so they do not change the output.
    expect(output[0].code).not.toContain('DEP');
    expect(output[0].code).toContain('ext');
  } finally {
    await build.close();
  }
});
