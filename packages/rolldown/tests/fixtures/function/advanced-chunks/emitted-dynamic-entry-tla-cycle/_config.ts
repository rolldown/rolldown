import { defineTest } from 'rolldown-tests';

export default defineTest({
  config: {
    input: './main.js',
    plugins: [
      {
        name: 'emit-dynamic-entry',
        buildStart() {
          this.emitFile({ type: 'chunk', id: './lazy.js', name: 'lazy' });
        },
      },
    ],
    output: {
      entryFileNames: '[name].js',
      chunkFileNames: '[name].js',
      advancedChunks: {
        groups: [{ name: 'app', test: /(?:bootstrap|shared)\.js$/ }],
      },
    },
  },
  async afterTest() {
    await import('./_test.mjs');
  },
});
