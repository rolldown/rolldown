import { defineTest } from 'rolldown-tests';
import { esmExternalRequirePlugin } from 'rolldown/plugins';
import { expect } from 'vitest';

const external = [
  'data:text/javascript,export default "single\'quote";',
  "data:text/javascript,export default 'double\"quote';",
  'data:text/javascript,export default "back\\\\slash";',
  'data:text/javascript,\nexport default "line-break";\r',
];

export default defineTest({
  config: {
    output: {
      format: 'esm',
    },
    plugins: [esmExternalRequirePlugin({ external })],
  },
  async afterTest(output) {
    expect(new Set(output.output[0].imports)).toEqual(new Set(external));

    const { values } = await import('./dist/main.js' as string);
    expect(values).toEqual(["single'quote", 'double"quote', 'back\\slash', 'line-break']);
  },
});
