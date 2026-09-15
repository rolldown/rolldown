import { defineTest } from 'rolldown-tests';
import type { Plugin } from 'rolldown';
import { expect } from 'vitest';

// Companion to ../issue-10186: a rooted-but-drive-less id that lives under
// `preserveModulesRoot` must get the root stripped like any absolute path.
//
// Node (and Rollup) treat `/src/extra.js` as absolute — anchored to the
// current drive on Windows — and `preserveModulesRoot` is normalized against
// the cwd, so `/src` becomes `<cwd drive>:\src` there. The rooted id must be
// anchored the same way for the strip to apply; without that, Windows emitted
// `src/extra.js` (or crashed, before the issue-10186 fix) while posix emitted
// `extra.js`.

const ROOTED_ID = '/src/extra.js';

const keepRootedId: Plugin = {
  name: 'keep-rooted-id',
  resolveId(id) {
    if (id === ROOTED_ID) return id;
  },
  load(id) {
    // A default-exported string keeps the module from being const-inlined
    // away, so it still gets its own preserved chunk.
    if (id === ROOTED_ID) return `export default ${JSON.stringify(ROOTED_ID)}`;
  },
};

export default defineTest({
  config: {
    input: 'main.js',
    plugins: [keepRootedId],
    output: {
      preserveModules: true,
      preserveModulesRoot: '/src',
    },
  },
  afterTest: (output) => {
    const fileNames = output.output.map((chunk) => chunk.fileName);
    // `/src/extra.js` is under `preserveModulesRoot` (`/src`), so the root is
    // stripped — identical on every platform.
    expect(fileNames).toContain('extra.js');
    // The entry lives outside `preserveModulesRoot`; its path nests under this
    // machine's absolute path, so only assert the shared invariant.
    for (const name of fileNames) {
      expect(name.startsWith('/')).toBe(false);
      expect(name.startsWith('\\')).toBe(false);
    }
  },
});
