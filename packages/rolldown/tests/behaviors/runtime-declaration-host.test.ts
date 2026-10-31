import path from 'node:path';
import { rolldown } from 'rolldown';
import { expect, test } from 'vitest';

for (const extension of ['.d.ts', '.d.mts', '.d.cts']) {
  test.each([false, true])(
    `keeps runtime out of ${extension} chunks with a JavaScript consumer: %s`,
    async (hasJavaScriptConsumer) => {
      // Declaration plugins turn types into fake JavaScript, then discard generated runtime
      // statements in renderChunk. A declaration chunk cannot host helpers for other chunks.
      const binding = `/binding${extension}`;
      const a = `/a${extension}`;
      const b = `/b${extension}`;
      const modules: Record<string, string> = {
        [a]: `export { Type } from './binding${extension}';`,
        [b]: `export { Type } from './binding${extension}';`,
        [binding]: `import * as types from './types${extension}'; export const Type = types;`,
        [`/types${extension}`]: 'export const X = 1;',
        '/entry.js': `import { Type } from './binding${extension}';
          import * as logger from './logger.js'; console.log(Type, logger);`,
        '/logger.js': 'export const log = () => 1;',
      };
      const bundle = await rolldown({
        input: hasJavaScriptConsumer ? ['/entry.js', a, b] : [a, b],
        plugins: [
          {
            name: 'fake-declarations',
            resolveId(id, importer) {
              return importer && id.startsWith('.')
                ? path.posix.resolve(path.posix.dirname(importer), id)
                : id;
            },
            load: (id) => modules[id],
          },
        ],
      });
      try {
        const { output } = await bundle.generate({ format: 'es' });
        const runtimeChunk = output.find(
          (chunk) => chunk.type === 'chunk' && '\0rolldown/runtime.js' in chunk.modules,
        );
        expect(runtimeChunk).toBeDefined();
        if (runtimeChunk?.type === 'chunk') {
          expect(Object.keys(runtimeChunk.modules).some((id) => id.endsWith(extension))).toBe(
            false,
          );
        }
      } finally {
        await bundle.close();
      }
    },
  );
}
