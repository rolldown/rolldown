import path from 'node:path';
import { defineTest } from 'rolldown-tests';
import { expect } from 'vitest';

export default defineTest({
  config: {
    plugins: [
      {
        name: 'test-input-format',
        load(id) {
          if (id === '\0virtual:unknown.js') {
            return 'console.log("virtual module")';
          }
        },
        moduleParsed(moduleInfo) {
          const filename = path.basename(moduleInfo.id);
          switch (filename) {
            case 'main.js':
              // main.js has both import and export, so it's ESM
              expect(moduleInfo.inputFormat).toBe('es');
              break;
            case 'esm.js':
              // esm.js has export, so it's ESM
              expect(moduleInfo.inputFormat).toBe('es');
              break;
            case 'cjs.js':
              // cjs.js uses module.exports, so it's CommonJS
              expect(moduleInfo.inputFormat).toBe('cjs');
              break;
            case 'side-effect.js':
              // side-effect.js has no module syntax, but parent package.json has "type": "module"
              // so it's detected as ESM based on the package.json type field
              expect(moduleInfo.inputFormat).toBe('es');
              break;
            case 'unknown.js':
              // unknown.js has no module syntax and its package.json has no "type" field
              // so it's detected as "unknown"
              expect(moduleInfo.inputFormat).toBe('unknown');
              break;
          }

          // Virtual module: no package.json context, no module syntax -> unknown
          if (moduleInfo.id === '\0virtual:unknown.js') {
            expect(moduleInfo.inputFormat).toBe('unknown');
          }

          // data: URL: no package.json context, no module syntax -> unknown
          if (moduleInfo.id.startsWith('data:')) {
            expect(moduleInfo.inputFormat).toBe('unknown');
          }
        },
      },
    ],
  },
});
