import { defineTest } from 'rolldown-tests';
import type { Plugin } from 'rolldown';
import { expect } from 'vitest';

// Companion to ../issue-10186: a rooted id (`/favicon`) that a plugin marks
// **external**. External modules never produce preserved chunks, so they must
// not participate in the input-base computation (Rollup's
// `getIncludedModules` keeps internal modules only). If the external id were
// anchored like an internal one, the input base would collapse to the
// filesystem root and the entry would be emitted nested under this machine's
// absolute path (`home/user/…/main.js`) instead of as plain `main.js`.

const EXTERNAL_ID = '/favicon';

const externalRootedId: Plugin = {
  name: 'external-rooted-id',
  resolveId(id) {
    if (id === EXTERNAL_ID) return { id, external: true };
  },
};

export default defineTest({
  config: {
    input: 'main.js',
    plugins: [externalRootedId],
    output: {
      preserveModules: true,
    },
  },
  afterTest: (output) => {
    // The external id must leave the input base at the entry's directory, so
    // the sole chunk is exactly `main.js` — on every platform.
    expect(output.output.map((chunk) => chunk.fileName)).toStrictEqual(['main.js']);
  },
});
