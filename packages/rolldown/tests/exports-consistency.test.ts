import { describe, expect, test } from 'vitest';
import pkg from '../package.json';
import browserPkg from '../../browser/package.json';

describe('package.json exports consistency', () => {
  test('publishConfig.exports keys match exports keys', () => {
    const exportsKeys = Object.keys(pkg.exports).sort();
    const publishExportsKeys = Object.keys(pkg.publishConfig.exports).sort();

    expect(exportsKeys).toStrictEqual(publishExportsKeys);
  });

  test('browser package.json exports keys match normal package exports keys', () => {
    const exportsKeys = Object.keys(browserPkg.exports).sort();
    const publishExportsKeys = Object.keys(pkg.publishConfig.exports).sort();

    expect(exportsKeys).toStrictEqual(publishExportsKeys);
  });

  test('browser package.json imports keys match normal package imports keys except parallel plugin worker', () => {
    const importsKeys = Object.keys(pkg.imports)
      .filter((key) => key !== '#parallel-plugin-worker')
      .sort();
    const browserImportsKeys = Object.keys(browserPkg.imports).sort();

    expect(browserImportsKeys).toStrictEqual(importsKeys);
  });
});
