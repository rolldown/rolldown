import path from 'node:path';
import { expect } from 'vitest';
import { defineTest } from 'rolldown-tests';

// Ported from rspack `tests/rspack-test/configCases/lazy-barrel/basic/mixed-barrel`.
//
// barrel/index.js mixes a default re-export, a star re-export, an aliased named
// re-export and an own export:
//   export { default as a } from "./a";  // default re-export -> a.js
//   export * from "./b";                 // star -> b.js (also exports `b`)
//   export { value as b } from "./c";    // named re-export `b` -> c.js.value
//   export const d = 'd';                // own export
//
// The requested specifier `b` is satisfied by the EXPLICIT named re-export
// (c.js.value), which shadows the `export * from "./b"` `b`. Because `b` is found
// in named exports, the star export is NOT searched, so neither the default
// re-export target (a.js) nor the star target (b.js) is loaded.
//
// DEVIATION FROM RSPACK: rspack's basic test imports `{ b as c, d }` and still
// skips a.js + b.js. rolldown, by contrast, treats any use of an OWN export
// (`d`) as forcing the barrel to execute, which loads ALL of its import records
// (documented behavior; see treeshake-behavior/case-own-export*). So importing
// `d` here would defeat the skip. To isolate the named-resolution skip this
// scenario demonstrates, main.js imports only `{ b as c }`.
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
    // `b` resolves to the named re-export of c.js.value -> c.js loaded.
    expect(relativeIds).toContain('src/main.js');
    expect(relativeIds).toContain('src/mixed-barrel/index.js');
    expect(relativeIds).toContain('src/mixed-barrel/c.js');
    // Unused default re-export target is skipped (rspack skips this too).
    expect(relativeIds).not.toContain('src/mixed-barrel/a.js');
    // `export * from "./b"` target is skipped because `b` was found in named
    // exports, so star exports are never searched (rspack skips this too).
    expect(relativeIds).not.toContain('src/mixed-barrel/b.js');
    expect(transformedIds.length).toBe(3);

    await import('./_test.mjs');
  },
});
