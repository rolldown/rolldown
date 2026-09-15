import { createRequire } from 'node:module';
import { isWasiTest } from 'rolldown-tests/utils';
import { expect, test } from 'vitest';

// A stale `dist` would silently turn the WASI lane into a native run or vice versa, so assert the suite runs on the binding flavor it is configured for.
const WASM_GLUE = ['@napi-rs/wasm-runtime', '@tybys/wasm-util', '@emnapi/'];
// Both native layouts use this filename: the dist-local addon and the `@rolldown/binding-*` package whose `main` it is. Unrelated addons like fsevents must not match.
const NATIVE_BINDING = /\/rolldown-binding\.[^/]+\.node$/;

test('loads the binding the suite is configured for', async () => {
  await import('rolldown');

  const cached = Object.keys(createRequire(import.meta.url).cache).map((id) =>
    id.replaceAll('\\', '/'),
  );
  const wasmGlue = cached.filter((id) => WASM_GLUE.some((pkg) => id.includes(pkg)));
  const nativeBinding = cached.filter((id) => NATIVE_BINDING.test(id));

  if (isWasiTest) {
    const hint =
      'ROLLDOWN_WASI_TEST=1 but the wasm binding was not loaded. Run ' +
      '`just build-rolldown-wasi` so `packages/rolldown/dist` holds the wasm binding instead of a ' +
      '`.node` file.';
    expect(wasmGlue, hint).not.toEqual([]);
    expect(nativeBinding, hint).toEqual([]);
  } else {
    const hint =
      'the wasm binding was loaded without ROLLDOWN_WASI_TEST=1. Run the suite through ' +
      '`just test-wasi`, which sets the flag the wasm-specific skips are keyed on.';
    expect(nativeBinding, hint).not.toEqual([]);
    expect(wasmGlue, hint).toEqual([]);
  }
});
