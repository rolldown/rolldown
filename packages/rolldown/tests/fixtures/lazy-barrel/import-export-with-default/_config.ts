import path from 'node:path';
import { expect } from 'vitest';
import { defineTest } from 'rolldown-tests';

const transformedIds: string[] = [];

export default defineTest({
  config: {
    experimental: {
      lazyBarrel: true,
    },
    treeshake: {
      moduleSideEffects(id) {
        if (/barrel[\\/](index|a)\.js$/.test(id)) {
          return false;
        }
        return true;
      },
    },
    plugins: [
      {
        name: 'track-transforms',
        transform(_, id) {
          if (id.startsWith('\0')) {
            return;
          }
          transformedIds.push(id);
        },
      },
    ],
  },
  afterTest: () => {
    const relativeIds = transformedIds.map((id) =>
      path.relative(import.meta.dirname, id).replace(/\\/g, '/'),
    );
    // barrel/index.js uses `import { a } from './a.js'; export { a }` (import-then-export)
    // and `const b = a(); export default b` (local export using the imported binding).
    // When `default` is imported, barrel must execute, which requires `a` from a.js.
    // Since the import record for a.js is NOT a direct re-export record,
    // it must be loaded with its specifiers so the barrel can access `a`.
    // a.js is itself a barrel (`export { a } from './aa.js'`), so if the `a` specifier
    // was correctly requested, aa.js must also be loaded.
    expect(relativeIds).toContain('main.js');
    expect(relativeIds).toContain('barrel/index.js');
    expect(relativeIds).toContain('barrel/a.js');
    expect(relativeIds).toContain('barrel/aa.js');
    expect(transformedIds.length).toBe(4);
  },
});
