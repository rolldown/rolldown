import fs from 'node:fs';
import path from 'node:path';
import { defineTest } from 'rolldown-tests';
import { expect } from 'vitest';

const order: string[] = [];

export default defineTest({
  config: {
    plugins: [
      {
        name: 'emit-asset',
        load(id) {
          if (!id.endsWith('main.js')) return;
          const referenceId = this.emitFile({
            type: 'asset',
            name: 'asset.txt',
            source: fs.readFileSync(path.join(import.meta.dirname, 'asset.txt')),
          });
          return `export const url = import.meta.ROLLUP_FILE_URL_${referenceId};`;
        },
      },
      {
        name: 'normal',
        resolveFileUrl: {
          handler() {
            order.push('normal');
            return `'normal'`;
          },
        },
      },
      {
        name: 'post',
        resolveFileUrl: {
          order: 'post',
          handler() {
            order.push('post');
            return `'post'`;
          },
        },
      },
      {
        name: 'pre',
        resolveFileUrl: {
          order: 'pre',
          handler() {
            order.push('pre');
            return `'pre'`;
          },
        },
      },
    ],
  },
  afterTest: async (output) => {
    const chunk = output.output.find((o) => o.type === 'chunk')!;
    // `pre` runs first despite being declared last, returns non-null, and nothing
    // after it is consulted.
    expect(order).toEqual(['pre']);
    expect(chunk.code).toContain('"pre"');
    expect(chunk.code).not.toContain('"normal"');
    expect(chunk.code).not.toContain('"post"');
  },
});
