import { defineTest } from 'rolldown-tests';
import { expect } from 'vitest';

export default defineTest({
  config: {
    experimental: { importAttributes: true },
    plugins: [
      {
        name: 'test',
        resolveId(id) {
          if (id.includes('other')) return id;
        },
        load(id) {
          if (id.endsWith('./other?attributes=%7B%22type%22%3A%22type1%22%7D'))
            return 'export const other1 = 1';
          if (id.endsWith('./other?attributes=%7B%22type%22%3A%22type2%22%7D'))
            return 'export const other2 = 2';
        },
      },
    ],
  },
  async afterTest() {
    // @ts-ignore
    const { other1, other2 } = await import('./dist/main.js');
    expect(other1).not.toBe(other2);
  },
});
