import { describe, expect, test } from 'vitest';
import pkg from '../package.json';

describe('package.json exports consistency', () => {
  test('publishConfig.exports keys match exports keys', () => {
    const exportsKeys = Object.keys(pkg.exports).sort();
    const publishExportsKeys = Object.keys(pkg.publishConfig.exports).sort();

    expect(exportsKeys).toStrictEqual(publishExportsKeys);
  });
});
