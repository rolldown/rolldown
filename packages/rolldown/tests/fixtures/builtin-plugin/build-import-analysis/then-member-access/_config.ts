import type { OutputChunk } from 'rolldown';
import { defineTest } from 'rolldown-tests';
import { viteBuildImportAnalysisPlugin } from 'rolldown/experimental';
import { expect } from 'vitest';

export default defineTest({
  config: {
    input: './main.js',
    treeshake: {
      moduleSideEffects: false,
    },
    plugins: [
      {
        // insert some dummy runtime flag to assert the runtime behavior
        name: 'insert_dummy_flag',
        transform(code) {
          let runtimeCode = `const __VITE_PRELOAD__ = [];`;
          return {
            code: runtimeCode + code,
          };
        },
      },
      viteBuildImportAnalysisPlugin({
        preloadCode: `export const __vitePreload = (v) => { return v() };`,
        insertPreload: true,
        optimizeModulePreloadRelativePaths: false,
        renderBuiltUrl: false,
        isRelativeBase: false,
      }),
    ],
  },
  async afterTest(output) {
    await import('./assert.mjs');
    output.output.forEach((item) => {
      if (item.type === 'chunk' && item.name === 'main') {
        // Check that the .then() callback is preserved
        expect(item.code).to.includes('.then((m) => m.foo)');
        expect(item.code).to.includes('.then((m) => m.bar)');
        expect(item.code).to.includes('.then((m) => m.nested.value)');
      }
    });
    
    // Check that lib chunk contains only the used exports
    const libChunk = output.output.find((item) => 
      item.type === 'chunk' && item.fileName.includes('lib')
    ) as OutputChunk | undefined;
    
    expect(libChunk).toBeDefined();
    if (libChunk) {
      // Verify expected exports are present
      expect(libChunk.code).toContain('foo');
      expect(libChunk.code).toContain('bar');
      expect(libChunk.code).toContain('nested');
    }
  },
});
