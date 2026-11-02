import path from 'node:path';

import { defineTest } from 'rolldown-tests';
import { expect } from 'vitest';

export default defineTest({
  config: {
    input: path.join(__dirname, './main.js'),
    plugins: [
      {
        name: 'test-plugin',
        generateBundle(_options, bundle) {
          const chunk = bundle['main.js'];
          expect(chunk.type).toBe('chunk');
          if (chunk.type !== 'chunk') return;
          const modules = Object.values(chunk.modules);
          expect(modules.length).toBe(1);
          const [module] = modules;
          expect(module.code).toContain('🚀');
          // `renderedLength` counts UTF-16 code units like `code.length`, not UTF-8 bytes.
          expect(module.renderedLength).toBe(module.code!.length);
          expect(module.renderedLength).not.toBe(Buffer.byteLength(module.code!));
        },
      },
    ],
  },
});
