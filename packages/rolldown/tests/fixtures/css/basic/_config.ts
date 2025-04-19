import fs from 'node:fs';
import { fileURLToPath } from 'node:url';
import { expect } from 'vitest'
import { defineTest } from 'rolldown-tests'
import { getOutputAsset } from '../../../src/utils';

export default defineTest({
  config: {
    input: './index.tsx',
  },
  async afterTest(output) {
    const assets = getOutputAsset(output)
    for (const asset of assets) {
      if (!asset.fileName.endsWith('.css')) {
        continue;
      }
      await expect(asset.source).toMatchFileSnapshot(
        fileURLToPath(new URL(`${asset.fileName}.snap`, import.meta.url)),
      )
    }
  },
})
