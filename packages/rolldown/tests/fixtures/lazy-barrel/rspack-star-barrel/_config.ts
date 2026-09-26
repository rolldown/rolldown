import path from 'node:path';
import { expect } from 'vitest';
import { defineTest } from 'rolldown-tests';

// Ported from rspack `tests/rspack-test/configCases/lazy-barrel/basic/star-barrel`.
//
// barrel/index.js exposes one named re-export, two star re-exports and an own
// export:
//   export { c } from "./c";  // named re-export `c` -> c.js
//   export * from "./a";      // star -> a.js
//   export * from "./b";      // star -> b.js
//   export const d = 'd';     // own export (not requested here)
//
// main.js imports { b }. `b` is NOT an explicit named re-export, so it cannot be
// resolved without searching the star re-exports. rolldown therefore loads ALL
// star targets (a.js AND b.js) to resolve it, while the unused named re-export
// target c.js is skipped. This matches rspack, which skips only star-barrel/c.js.
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
    // `b` is reachable only through `export *`, so every star target loads.
    expect(relativeIds).toContain('src/main.js');
    expect(relativeIds).toContain('src/star-barrel/index.js');
    expect(relativeIds).toContain('src/star-barrel/a.js');
    expect(relativeIds).toContain('src/star-barrel/b.js');
    // The unused named re-export `c` is skipped (rspack skips this too).
    expect(relativeIds).not.toContain('src/star-barrel/c.js');
    expect(transformedIds.length).toBe(4);

    await import('./_test.mjs');
  },
});
