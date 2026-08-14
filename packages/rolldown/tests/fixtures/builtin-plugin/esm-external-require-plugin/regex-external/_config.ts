import { defineTest } from 'rolldown-tests';
import { esmExternalRequirePlugin } from 'rolldown/plugins';
import { expect } from 'vitest';

export default defineTest({
  config: {
    output: {
      format: 'esm',
    },
    plugins: [esmExternalRequirePlugin({ external: [/^my-lib-/] })],
  },
  async afterTest(output) {
    const code = output.output[0].code;
    expect(code).toMatch(/import \* as [$\w]+ from "my-lib-a"/);
    expect(code).toMatch(/import \* as [$\w]+ from "my-lib-b"/);
  },
});
