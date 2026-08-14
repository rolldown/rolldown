import { defineTest } from 'rolldown-tests';
import { esmExternalRequirePlugin } from 'rolldown/plugins';
import { expect } from 'vitest';

export default defineTest({
  config: {
    output: {
      format: 'cjs',
    },
    plugins: [esmExternalRequirePlugin({ external: ['ext'] })],
  },
  async afterTest(output) {
    const code = output.output[0].code;
    expect(code).toContain('require("ext")');
  },
});
