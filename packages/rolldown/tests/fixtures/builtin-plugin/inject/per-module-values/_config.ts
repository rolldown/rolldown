import nodePath from 'node:path';
import { defineTest } from 'rolldown-tests';
import { injectPlugin } from 'rolldown/plugins';

const resolve = (file: string) => nodePath.join(import.meta.dirname, file);

export default defineTest({
  config: {
    external: ['node:assert'],
    plugins: [
      // a framework injects its own `foo` into app code
      injectPlugin({ foo: [resolve('app-foo.js'), 'foo'] }, { exclude: /[\\/]vendor[\\/]/ }),
      // the user injects another `foo` everywhere; modules that already import one keep it
      injectPlugin({ foo: [resolve('shim-foo.js'), 'foo'] }),
    ],
  },
  afterTest: async () => {
    await import('./_test.mjs');
  },
});
