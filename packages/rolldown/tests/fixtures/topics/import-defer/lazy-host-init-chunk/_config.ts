import { defineTest } from 'rolldown-tests';
import { getOutputAsset, getOutputChunk } from 'rolldown-tests/utils';
import { expect } from 'vitest';

const assetSource = 'export const owner = "plugin";\n';
const rendered = new Map<string, { code: string; imports: string[] }>();
const hashed: string[] = [];

// `b.js` imports the lazy host `a.js`, then `y.js`. The chunk of `b.js` imports an init chunk
// right after the chunk of `a.js`, so `a` runs before `y`. Without the asset of the plugin, the
// init chunk gets the name `a.init.js`.
export default defineTest({
  sequential: true,
  config: {
    experimental: {
      importDefer: true,
    },
    output: {
      entryFileNames: '[name].js',
      chunkFileNames: '[name].js',
    },
    plugins: [
      {
        name: 'test-plugin',
        buildStart() {
          this.emitFile({ type: 'asset', fileName: 'a.init.js', source: assetSource });
        },
        renderChunk(code, chunk) {
          rendered.set(chunk.fileName, { code, imports: chunk.imports });
          return null;
        },
        augmentChunkHash(chunk) {
          hashed.push(chunk.fileName);
        },
      },
    ],
  },
  afterTest: async (output) => {
    // The asset keeps its name and its content. The init chunk gets a number in its name.
    const asset = getOutputAsset(output).find((item) => item.fileName === 'a.init.js');
    expect(asset?.source).toBe(assetSource);
    const chunks = getOutputChunk(output);
    const init = chunks.find((chunk) => chunk.fileName === 'a2.init.js');
    expect(init?.imports).toStrictEqual(['a.js']);
    expect(init?.code).toMatch(/from "\.\/a\.js";\ninit_a\(\);\n$/);

    // The hooks see the init chunk, and the importer lists it in its imports.
    expect(rendered.get('a2.init.js')?.code).toMatch(/from "\.\/a\.js";\n+init_a\(\);\n$/);
    expect(hashed).toContain('a2.init.js');
    const b = chunks.find((chunk) => chunk.fileName === 'b.js');
    expect(b?.imports).toStrictEqual(['a.js', 'a2.init.js', 'y.js']);
    expect(rendered.get('b.js')?.imports).toStrictEqual(b?.imports);

    const log: string[] = ((globalThis as { __initLog?: string[] }).__initLog = []);
    const { access } = await import('./dist/main.js' as string);
    expect(await access()).toBe('b,a,a');
    expect(log).toStrictEqual(['a', 'y', 'b']);
    delete (globalThis as { __initLog?: string[] }).__initLog;
  },
});
