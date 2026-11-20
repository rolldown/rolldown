import { defineTest } from 'rolldown-tests';
import { expect } from 'vitest';

export default defineTest({
  config: {
    input: { right: './right.js', right2: './right.js' },
    plugins: [
      {
        name: 'emit-left',
        buildStart() {
          this.emitFile({ type: 'chunk', id: './left.js', fileName: 'shared.js' });
          this.emitFile({ type: 'chunk', id: './left.js', fileName: 'right.js' });
        },
      },
    ],
    output: {
      chunkFileNames: '[name].js',
    },
  },
  afterTest: async (output) => {
    expect(output.output.find((file) => file.fileName === 'shared2.js')).toMatchObject({
      type: 'chunk',
      name: 'shared',
    });
    expect(output.output.find((file) => file.fileName === 'right3.js')).toMatchObject({
      type: 'chunk',
      name: 'right',
    });
    expect(output.output.find((file) => file.fileName === 'right2.js')).toMatchObject({
      type: 'chunk',
      name: 'right2',
    });
    await import('./_test.mjs');
  },
});
