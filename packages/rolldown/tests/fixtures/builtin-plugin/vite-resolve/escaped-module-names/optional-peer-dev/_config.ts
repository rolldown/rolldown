import { defineTest } from 'rolldown-tests';
import { expect } from 'vitest';
import { createResolver, parentDep, peerDep, peerModuleId } from '../utils';

export default defineTest({
  config: {
    input: peerModuleId,
    output: { entryFileNames: 'main.js' },
    plugins: [
      {
        name: 'resolve-optional-peer-dep-fixture',
        resolveId(id) {
          if (id === peerModuleId) return id;
        },
      },
      createResolver(import.meta.dirname, false, false),
    ],
  },
  async afterTest(output) {
    await expect(
      import(`data:text/javascript,${encodeURIComponent(output.output[0].code)}`),
    ).rejects.toThrow(new Error(`Could not resolve "${peerDep}" imported by "${parentDep}".`));
  },
});
