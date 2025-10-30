import { stripAnsi } from 'consola/utils';
import * as path from 'node:path';
import { defineTest } from 'rolldown-tests';
import { id, include } from 'rolldown/filter';
import { expect } from 'vitest';

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
          filter: [include(id(/\.js$/))],
          handler(id) {
            if (id.includes('foo.js')) {
              return path.resolve(import.meta.dirname, cleanUrl(id));
            }
          },
        },
      },
    ],
  },
  catchError(err: any) {
    expect(stripAnsi(err.toString())).toContain(
      `[UNLOADABLE_DEPENDENCY] Error: Could not load foo.js?test=hello`,
    );
  },
});
