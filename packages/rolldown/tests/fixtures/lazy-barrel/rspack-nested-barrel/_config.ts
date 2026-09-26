import path from 'node:path';
import { expect } from 'vitest';
import { defineTest } from 'rolldown-tests';

// Ported from rspack `tests/rspack-test/configCases/lazy-barrel/basic/nested-barrel`.
//
// barrel/index.js re-exports from nested barrels:
//   export { a } from "./a";   // a.js: import { b as a } from "./b"; export { a };
//   export { c } from "./c";   //       export { c } from "./c";
// main.js does `import * as nested from "./nested-barrel"` and uses `nested.a`.
//
// DIVERGENCE FROM RSPACK (intentional, documented): rspack performs namespace
// member-usage analysis for `import * as ns` and therefore skips nested-barrel/c.js
// (only `nested.a` is used). rolldown intentionally does NOT skip namespace
// members — `import * as ns`, entry files, `import()` and `require()` all cause a
// barrel to load ALL of its exports (see docs/in-depth/lazy-barrel-optimization.md
// "Limitations"). So every re-export target IS loaded here.
//
// The expectation is ADAPTED to rolldown's behavior: assert the EXACT full
// loaded set (count === 5, nothing skipped). This is a precise structural check
// that encodes the documented `import * as` = load-all rule, not merely "it built".
const transformedIds: string[] = [];

export default defineTest({
  config: {
    input: { main: './src/main.js' },
    experimental: {
      lazyBarrel: true,
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
        },
      },
    ],
  },
  afterTest: async () => {
    const relativeIds = transformedIds.map((id) =>
      path.relative(import.meta.dirname, id).replace(/\\/g, '/'),
    );
    // `import * as nested` loads every export of the barrel (and, recursively,
    // every export reachable from it), so all four module files plus the entry
    // are loaded.
    expect(relativeIds).toContain('src/main.js');
    expect(relativeIds).toContain('src/nested-barrel/index.js');
    expect(relativeIds).toContain('src/nested-barrel/a.js');
    expect(relativeIds).toContain('src/nested-barrel/b.js');
    // Unlike rspack (which skips it), rolldown loads c.js for `import * as`.
    expect(relativeIds).toContain('src/nested-barrel/c.js');
    expect(transformedIds.length).toBe(5);

    await import('./_test.mjs');
  },
});
