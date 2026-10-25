import nodePath from 'node:path';
import { defineTest } from 'rolldown-tests';

export default defineTest({
  config: {
    transform: {
      inject: {
        // import { useKey } from '<this folder>/composable.js'
        useKey: [nodePath.join(import.meta.dirname, 'composable.js'), 'useKey'],
      },
      // skip `vendor/`, except `vendor/layer/`
      injectFilter: { exclude: /[\\/]vendor[\\/](?!layer[\\/])/ },
    },
    external: ['node:assert'],
    plugins: [
      {
        name: 'virtual',
        resolveId(id) {
          if (id === 'virtual:dep') {
            return '\0virtual:dep';
          }
        },
        load(id) {
          if (id === '\0virtual:dep') {
            return `export const virtualHasUseKey = () => typeof useKey !== 'undefined'`;
          }
        },
      },
    ],
  },
  afterTest: async () => {
    await import('./_test.mjs');
  },
});
