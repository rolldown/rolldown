import path from 'node:path';
import { defineTest } from 'rolldown-tests';
import { expect } from 'vitest';

const entry = path.join(__dirname, './main.js');

export default defineTest({
  config: {
    input: entry,
    platform: 'browser',
    plugins: [
      {
        name: 'return-this-resolve',
        async resolveId(source, importer, options) {
          if (!importer) {
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
    expect(output.output[0].code).toBe('');
  },
});
