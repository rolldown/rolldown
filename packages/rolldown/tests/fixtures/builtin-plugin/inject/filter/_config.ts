import nodePath from 'node:path';
import { defineTest } from 'rolldown-tests';
import { injectPlugin } from 'rolldown/plugins';

export default defineTest({
  config: {
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
      injectPlugin(
        { useKey: [nodePath.join(import.meta.dirname, 'composable.js'), 'useKey'] },
        // skip `vendor/`, except `vendor/layer/`
        { exclude: /[\\/]vendor[\\/](?!layer[\\/])/ },
      ),
    ],
  },
  afterTest: async () => {
    await import('./_test.mjs');
  },
});
