import { isWasiTest } from '@tests/runtime-flavor';
import { defineTest } from 'rolldown-tests';
import { expect } from 'vitest';

export default defineTest({
  // KNOWN: wasm/emnapi error boundary. On the WASI binding the original JS
  // error thrown by a plugin hook does not round-trip through the wasm
  // boundary: napi-rs re-creates it via emnapi's `napi_create_error`, so the
  // original stack frames (`at errorFn1/errorFn2`) and custom properties
  // (`extraProp`) are lost and `errors[0].code` degrades from PLUGIN_ERROR to
  // GenericFailure. Structured plugin-error propagation — the whole point of
  // this fixture — is not functional on wasm.
  skip: isWasiTest,
  config: {
    plugins: [
      {
        name: 'my-plugin',
        async load() {
          await errorFn1();
        },
      },
    ],
  },
  catchError(e: any) {
    expect(e).toBeInstanceOf(Error);
    expect(e.message).toContain('my-error');
    expect(e.message).toContain('at errorFn2');
    expect(e.message).toContain('at errorFn1');
    expect(e.errors[0]).toMatchObject({
      message: 'my-error',
      extraProp: 1234,
      code: 'PLUGIN_ERROR',
      plugin: 'my-plugin',
      hook: 'load',
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
