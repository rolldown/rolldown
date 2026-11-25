import path from 'node:path';
import { defineTest } from 'rolldown-tests';
import { expect } from 'vitest';

// The group's `test` matches `peer.js` and `vendor.js`, but its `name` leaves `peer.js` out, so
// `peer.js` stays in the chunk of `a` and `b`. A fold of `shared.js` into that chunk would make it
// and `vendor` import each other, and `vendor.js` would read `token` before `shared.js` declares
// it. The fold plans the groups before it moves anything, and `name` still runs once per module.
const nameCalls: string[] = [];

export default defineTest({
  config: {
    input: { a: path.join(__dirname, 'a.js'), b: path.join(__dirname, 'b.js') },
    output: {
      codeSplitting: {
        groups: [
          {
            test: /[\\/](peer|vendor)\.js$/,
            name: (id) => {
              nameCalls.push(path.basename(id));
              return id.endsWith('vendor.js') ? 'vendor' : null;
            },
            includeDependenciesRecursively: false,
          },
        ],
      },
    },
  },
  async afterTest() {
    const log: string[] = [];
    (globalThis as any).foldNameFnLog = log;
    const a = await import('./dist/a.js' as string);
    await import('./dist/b.js' as string);
    expect(log).toEqual(['peer:ok', 'a', 'b']);
    expect((await a.load()).value).toBe('ok');
    expect(nameCalls).toEqual(['peer.js', 'vendor.js']);
  },
});
