import path from 'node:path';
import { expect } from 'vitest';
import { defineTest } from 'rolldown-tests';

const transformedIds: string[] = [];

// barrel/index.js, barrel/c.js and barrel/f.js are marked as no side effects
const noSideEffectsPattern = /barrel[\\/](index|c|f)\.js$/;

export default defineTest({
  config: {
    experimental: {
      lazyBarrel: true,
    },
    treeshake: {
      moduleSideEffects(id) {
        if (noSideEffectsPattern.test(id)) {
          return false;
        }
        return true;
      },
    },
    plugins: [
      {
        name: 'track-transforms',
        transform(_, id) {
          // Skip virtual modules (like \0rolldown/runtime.js)
          if (id.startsWith('\0')) {
            return;
          }
          transformedIds.push(id);
          if (id.endsWith('d.js') || id.endsWith('g.js')) {
            return { moduleSideEffects: false };
          }
        },
      },
    ],
  },
  afterTest: () => {
    const relativeIds = transformedIds.map((id) =>
      path.relative(import.meta.dirname, id).replace(/\\/g, '/'),
    );
    // import { index } - own export `index` is used
    // `index` is barrel's own export, so barrel must be executed.
    // When barrel executes, ALL its import records must be loaded because
    // sideEffects can only be determined after transform hook.
    // This includes both imports and re-exports.
    // Barrel has `import { d, dd } from './d.js'; export { d, dd }` (import-then-export).
    // Since the import record for d.js is shared (not a direct `export { } from`),
    // d.js is loaded with its specifiers `{d, dd}`. d.js in turn re-exports `dd`
    // from dd.js, so dd.js is also loaded.
    // g.js is loaded because barrel imports `gg` from it.
    // g.js is a pure re-export barrel, so gg.js is loaded to resolve `gg`.
    expect(relativeIds).toContain('main.js');
    expect(relativeIds).toContain('../barrel/index.js');
    expect(relativeIds).toContain('../barrel/a.js');
    expect(relativeIds).toContain('../barrel/b.js');
    expect(relativeIds).toContain('../barrel/c.js');
    expect(relativeIds).toContain('../barrel/d.js');
    expect(relativeIds).toContain('../barrel/dd.js');
    expect(relativeIds).toContain('../barrel/e.js');
    expect(relativeIds).toContain('../barrel/f.js');
    expect(relativeIds).toContain('../barrel/g.js');
    expect(relativeIds).toContain('../barrel/gg.js');
    expect(transformedIds.length).toBe(11);
  },
});
