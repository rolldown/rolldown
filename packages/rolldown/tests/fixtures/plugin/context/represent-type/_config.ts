import { defineTest } from 'rolldown-tests';
import { expect } from 'vitest';

export default defineTest({
  config: {
    input: './main.js',
    plugins: [
      {
        name: 'represent-type-metadata',
        load(id) {
          if (id.endsWith('main.js')) {
            return { code: 'export default 1', representType: 'text' };
          }
        },
        transform(_code, id) {
          if (id.endsWith('main.js')) return { representType: 'base64' };
        },
        moduleParsed(info) {
          if (info.id.endsWith('main.js')) expect(info.representType).toBe('base64');
        },
      },
    ],
  },
});
