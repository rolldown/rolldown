import { defineTest } from 'rolldown-tests';

export default defineTest({
  config: {
    plugins: [
      {
        name: 'test-plugin',
        buildStart(_opt) {
          this.emitFile({
            type: 'chunk',
            id: './main.js',
            name: 'main',
            preserveSignature: false,
          });
        },
      },
    ],
    output: {},
  },
  afterTest: async (_output) => {
    await import('./_test.mjs');
  },
});
