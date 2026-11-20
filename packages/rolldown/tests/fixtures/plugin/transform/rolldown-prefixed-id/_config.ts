import { defineTest } from 'rolldown-tests';
import { expect } from 'vitest';

export default defineTest({
  config: {
    plugins: [
      {
        name: 'test-plugin',
        resolveId(id) {
          if (id === 'rolldown:value') {
            return id;
          }
        },
        load(id) {
          if (id === 'rolldown:value') {
            return 'export default __VALUE__;';
          }
        },
        transform(code, id) {
          if (id === 'rolldown:value') {
            return code.replace('__VALUE__', '42');
          }
        },
      },
    ],
  },
  afterTest(output) {
    expect(output.output[0].code).toContain('42');
    expect(output.output[0].code).not.toContain('__VALUE__');
  },
});
