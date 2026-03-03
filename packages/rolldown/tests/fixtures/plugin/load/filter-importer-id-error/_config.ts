import { importerId, include } from '@rolldown/pluginutils';
import { defineTest } from 'rolldown-tests';
import { expect } from 'vitest';

export default defineTest({
  config: {
    input: './main.js',
    plugins: [
      {
        name: 'test-plugin',
        load: {
          filter: [include(importerId(/main\.js$/))],
          handler(_id) {
            return null;
          },
        },
      },
    ],
  },
  catchError(e: any) {
    expect(e).toBeInstanceOf(Error);
    expect(e.message).toContain('importerId');
    expect(e.message).toContain('resolveId');
    expect(e.message).toContain('load');
  },
});
