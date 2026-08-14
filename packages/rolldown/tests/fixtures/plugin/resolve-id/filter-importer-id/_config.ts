import { importerId, include } from '@rolldown/pluginutils';
import { defineTest } from 'rolldown-tests';
import { expect, vi } from 'vitest';

const resolveIdFn = vi.fn();

export default defineTest({
  sequential: true,
  config: {
    input: './main.js',
    plugins: [
      {
        name: 'test-plugin',
        resolveId: {
          // Only match imports from main.js (exclude imports from foo.js)
          filter: [include(importerId(/main\.js$/))],
          handler(id, importer) {
            resolveIdFn(id, importer);
            return null;
          },
        },
      },
    ],
  },
  afterTest: () => {
    // main.js imports foo.js and bar.js (2 calls from main.js)
    // foo.js imports baz.js (0 calls because foo.js doesn't match the filter)
    // So we expect 2 calls total
    expect(resolveIdFn).toHaveBeenCalledTimes(2);
    expect(resolveIdFn).toHaveBeenCalledWith('./foo.js', expect.stringContaining('main.js'));
    expect(resolveIdFn).toHaveBeenCalledWith('./bar.js', expect.stringContaining('main.js'));
    resolveIdFn.mockReset();
  },
});
