import path from 'node:path';
import { defineTest } from 'rolldown-tests';
import { expect } from 'vitest';

const entry = path.join(__dirname, './main.js');

export default defineTest({
  config: {
    input: entry,
    plugins: [
      {
        name: 'test-plugin',
        outro: () => '/* Outro */',
      },
    ],
  },
  afterTest: (output) => {
    expect(output.output[0].code).toContain('/* Outro */');
  },
});
