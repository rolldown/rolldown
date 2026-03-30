import { defineTest } from 'rolldown-tests';
import { stripVTControlCharacters } from 'node:util';
import { expect } from 'vitest';

export default defineTest({
  config: {
    plugins: [
      {
        name: 'test-virtual',
        resolveId(id) {
          if (id === 'virtual-module') {
            return '\0virtual-module';
          }
        },
        // intentionally no load hook
      },
    ],
  },
  catchError(e: any) {
    for (const error of e.errors) {
      error.message = stripVTControlCharacters(error.message);
    }
    expect(e.errors).toEqual(
      expect.arrayContaining([
        expect.objectContaining({
          kind: 'UNLOADABLE_DEPENDENCY',
          message: expect.stringContaining(
            'This module seems to be a virtual module, but no plugin handled it via the load hook.',
          ),
        }),
      ]),
    );
  },
});
