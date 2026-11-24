import path from 'node:path';
import { defineTest } from 'rolldown-tests';
import { getOutputChunk } from 'rolldown-tests/utils';
import { expect } from 'vitest';

const asset = path.join(import.meta.dirname, 'asset.txt');

export default defineTest({
  config: {
    // The native asset plugin loads `asset.txt` with `moduleSideEffects: false`.
    moduleTypes: { '.txt': 'asset' },
    plugins: [
      {
        name: 'test-plugin',
        resolveId(source) {
          if (source === './asset.txt') return { id: asset, moduleSideEffects: true };
        },
        transform(code, id) {
          if (id === asset) return { code };
        },
      },
    ],
  },
  afterTest(output) {
    const [chunk] = getOutputChunk(output);
    expect(chunk.code).toContain('main');
    expect(chunk.code).not.toContain('asset');
  },
});
