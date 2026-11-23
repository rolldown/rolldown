import path from 'node:path';
import { defineTest } from 'rolldown-tests';
import { getOutputChunk } from 'rolldown-tests/utils';
import { expect } from 'vitest';

const asset = path.join(import.meta.dirname, 'asset.txt');
const seen: Record<string, unknown> = {};

// The shown value follows the hooks of the module in order, from JS and Rust plugins alike: the
// native asset `load` gives `false` after a JS `resolveId` gave `true`.
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
        transform(_code, id) {
          if (id === asset) seen.transform = this.getModuleInfo(id)!.moduleSideEffects;
        },
        buildEnd() {
          seen.buildEnd = this.getModuleInfo(asset)!.moduleSideEffects;
        },
      },
    ],
  },
  afterTest(output) {
    expect(seen).toStrictEqual({ transform: false, buildEnd: false });
    const [chunk] = getOutputChunk(output);
    expect(chunk.code).toContain('main');
    expect(chunk.code).not.toContain('asset');
  },
});
