import { join } from 'node:path';
import { defineTest } from 'rolldown-tests';
import { isWasiTest } from 'rolldown-tests/utils';
import { expect } from 'vitest';

export default defineTest({
  // Under the wasm binding the error arrives with a `wasm://` stack, without the `at errorFn1` / `at errorFn2` frames asserted below.
  skip: isWasiTest,
  config: {
    plugins: [
      {
        name: 'my-plugin',
        async transform(_, id) {
          // Skip virtual modules (like \0rolldown/runtime.js)
          if (id.startsWith('\0')) {
            return;
          }
          await errorFn1();
        },
      },
    ],
  },
  catchError(e: any) {
    expect(e.message).toContain('[plugin my-plugin]');
    expect(e.message).toContain('my-error');
    expect(e.message).toContain('at errorFn2');
    expect(e.message).toContain('at errorFn1');
    expect(e.errors[0]).toMatchObject({
      message: 'my-error',
      extraProp: 1234,
      code: 'PLUGIN_ERROR',
      plugin: 'my-plugin',
      hook: 'transform',
      id: join(import.meta.dirname, './main.js'),
    });
  },
});

async function errorFn1() {
  await Promise.resolve();
  await errorFn2();
}

async function errorFn2() {
  await Promise.resolve();
  throw Object.assign(new Error('my-error'), { extraProp: 1234 });
}
