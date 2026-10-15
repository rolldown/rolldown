import path from 'node:path';
import { expect } from 'vitest';
import { defineTest } from 'rolldown-tests';

// Ported from rspack `tests/rspack-test/configCases/lazy-barrel/basic/named-barrel`.
// rspack asserts a specific set of modules is never created; the rolldown-native
// equivalent records every loaded (transformed) id and asserts the exact
// loaded-vs-skipped set.
//
// barrel/index.js is a PURE re-export barrel (no own export):
//   export { a as b } from "./a";   // named re-export -> a.js
//   export { b as c } from "./b";   // named re-export -> b.js
//   import { c as cc } from "./c";  // import-then-export (shared record)
//   import { d as dd } from "./d";  // import-then-export (shared record)
//   export { cc, dd };
// main.js imports { b as a, cc }.
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
    // `b` resolves to the named re-export `a` from a.js  -> a.js loaded.
    // `cc` resolves to the import-then-export of c.js     -> c.js loaded.
    expect(relativeIds).toContain('src/main.js');
    expect(relativeIds).toContain('src/named-barrel/index.js');
    expect(relativeIds).toContain('src/named-barrel/a.js');
    expect(relativeIds).toContain('src/named-barrel/c.js');
    // `c` (re-export of b.js) is unused -> b.js skipped (rspack skips this too).
    expect(relativeIds).not.toContain('src/named-barrel/b.js');
    // `dd` (import-then-export of d.js) is unused and the barrel has no own
    // export forcing execution, so rolldown skips d.js as a lazy re-export.
    // (rspack loads d.js because it treats the plain `import { d as dd }` as
    // eager; rolldown is more aggressive here and still correct since dd is
    // never used.)
    expect(relativeIds).not.toContain('src/named-barrel/d.js');
    expect(transformedIds.length).toBe(4);

    await import('./_test.mjs');
  },
});
