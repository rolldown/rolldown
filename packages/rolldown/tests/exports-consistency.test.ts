import { describe, expect, test } from 'vitest';
import pkg from '../package.json';
import browserPkg from '../../browser/package.json';

describe('package.json exports consistency', () => {
  test('publishConfig.exports keys match exports keys', () => {
    const exportsKeys = Object.keys(pkg.exports).sort();
    const publishExportsKeys = Object.keys(pkg.publishConfig.exports).sort();

    expect(exportsKeys).toStrictEqual(publishExportsKeys);
  });

  test('browser package.json adds only its workerd-specific exports', () => {
    const exportsKeys = Object.keys(browserPkg.exports)
      .filter(
        (key) => key !== './workerd' && key !== './workerd/wasm' && key !== './workerd/wasm.wasm',
      )
      .sort();
    const publishExportsKeys = Object.keys(pkg.publishConfig.exports).sort();

    expect(exportsKeys).toStrictEqual(publishExportsKeys);
    expect(browserPkg.exports['./workerd']).toEqual({
      types: './dist/workerd.d.mts',
      workerd: './dist/workerd.browser.mjs',
      browser: './dist/workerd.browser.mjs',
      default: './dist/workerd.mjs',
    });
    expect(browserPkg.exports['./workerd/wasm']).toEqual({
      types: './dist/workerd-wasm.d.ts',
      workerd: './dist/rolldown-binding.wasm32-wasip1.wasm',
      default: './dist/rolldown-binding.wasm32-wasip1.wasm',
    });
    expect(browserPkg.exports['./workerd/wasm.wasm']).toEqual(browserPkg.exports['./workerd/wasm']);
  });
});
