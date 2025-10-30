import * as path from 'node:path';
import { defineTest } from 'rolldown-tests';
import { id, include } from 'rolldown/filter';

const postfixRE = /[?#].*$/;
export function cleanUrl(url: string): string {
  return url.replace(postfixRE, '');
}

export default defineTest({
  config: {
    plugins: [
      {
        name: 'test',
        resolveId: {
          filter: [include(id(/\.js$/, { cleanUrl: true }))],
          handler(id) {
            if (id.includes('foo.js')) {
              return path.resolve(import.meta.dirname, cleanUrl(id));
            }
          },
        },
      },
    ],
  },
});
