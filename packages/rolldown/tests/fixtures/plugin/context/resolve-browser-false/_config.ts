import { defineTest } from 'rolldown-tests';
import { expect } from 'vitest';

// Regression test: Returning the result of `this.resolve()` from `resolveId` should
// preserve `browser: false` handling (the module should be ignored/empty).
export default defineTest({
  config: {
    input: './main.js',
    platform: 'browser',
    plugins: [
      {
        name: 'probe',
        async resolveId(source, importer, options) {
          if (options?.isEntry || !importer) {
            return;
          }
          const resolved = await this.resolve(source, importer, {
            ...options,
            skipSelf: true,
          });
          return resolved;
        },
      },
    ],
  },
  afterTest(output) {
    expect(output.output).toHaveLength(1);
    expect(output.output[0].type).toBe('chunk');
    expect(output.output[0].code).toBe('');
  },
});
