import { rolldown } from 'rolldown';
import type { InputOptions, Plugin } from 'rolldown';
import { describe, expect, test } from 'vitest';

const fixturePlugin: Plugin = {
  name: 'inline-const-fixture',
  resolveId(id) {
    if (id === 'virtual:main' || id === 'virtual:foo') return '\0' + id;
  },
  load(id) {
    if (id === '\0virtual:main') {
      return `import { X } from 'virtual:foo';\nconsole.log(X);\nconsole.log(X);\nconsole.log(X);`;
    }
    if (id === '\0virtual:foo') {
      return `export const X = 'shared-constant';`;
    }
  },
};

async function bundle(optimization?: InputOptions['optimization']) {
  const b = await rolldown({
    input: 'virtual:main',
    optimization,
    plugins: [fixturePlugin],
  });
  const result = await b.generate({ format: 'esm' });
  await b.close();
  return result.output[0].code;
}

describe('optimization.inlineConst', () => {
  test('empty object is treated the same as omitting the option', async () => {
    const omitted = await bundle({ inlineConst: undefined });
    const empty = await bundle({ inlineConst: {} });
    expect(empty).toBe(omitted);
  });
});
