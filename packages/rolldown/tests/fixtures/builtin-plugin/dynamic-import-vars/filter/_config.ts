import { defineTest } from 'rolldown-tests';
import {
  viteDynamicImportVarsPlugin,
  viteImportGlobPlugin,
} from 'rolldown/experimental';

export default defineTest({
  config: {
    plugins: [
      viteDynamicImportVarsPlugin({
        exclude: [/main\.js$/],
      }),
      viteImportGlobPlugin(),
    ],
  },
  async afterTest() {
    await import('./assert.mjs');
  },
});
