import path from 'node:path';
import { defineTest } from 'rolldown-tests';
import { expect } from 'vitest';

const main = path.join(import.meta.dirname, 'main.js');
const data = path.join(import.meta.dirname, 'data.js');

// Without `experimental.importAttributes`, `this.resolve` drops the attributes of an internal
// module, while an external keeps them.
export default defineTest({
  config: {
    input: main,
    external: ['ext'],
    plugins: [
      {
        name: 'test-plugin',
        async transform(_code, id) {
          if (id !== main) return;
          const attributes = { type: 'foo' };
          expect(await this.resolve('./data.js', id, { attributes })).toMatchObject({
            id: data,
            attributes: {},
          });
          expect(await this.resolve('ext', id, { attributes })).toMatchObject({
            id: 'ext',
            attributes,
            external: true,
          });
        },
      },
    ],
  },
});
