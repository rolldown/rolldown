import path from 'node:path';
import { defineTest } from 'rolldown-tests';
import { viteDynamicImportVarsPlugin, viteImportGlobPlugin } from 'rolldown/experimental';
import { expect } from 'vitest';

export default defineTest({
  config: {
    plugins: [
      viteDynamicImportVarsPlugin({
        async resolver(id) {
          return id.replace(/^@(?:😀)?/, path.resolve(import.meta.dirname, 'dir'));
        },
      }),
      viteImportGlobPlugin(),
    ],
  },
  async afterTest() {
    const output = await import('./dist/main.js' as string);
    for (const load of output.loaders) {
      const before = output.calls;
      await expect(load('module')).resolves.toHaveProperty('default', 42);
      expect(output.calls).toBe(before + 1);
    }
  },
});
