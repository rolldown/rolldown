import path from 'node:path';
import { defineTest } from 'rolldown-tests';
import { getOutputChunk } from 'rolldown-tests/utils';
import { expect, vi } from 'vitest';

const asset = path.join(import.meta.dirname, 'asset.txt');
const fn = vi.fn();

// A value that a native hook returns after a write replaces the written value.
export default defineTest({
  config: {
    // The native asset plugin loads `asset.txt` with `moduleSideEffects: false`.
    moduleTypes: { '.txt': 'asset' },
    plugins: [
      {
        name: 'test-plugin',
        load: {
          order: 'pre',
          handler(id) {
            if (id !== asset) return;
            this.getModuleInfo(id)!.moduleSideEffects = 'no-treeshake';
            expect(this.getModuleInfo(id)!.moduleSideEffects).toBe('no-treeshake');
          },
        },
        transform(_code, id) {
          if (id !== asset) return;
          expect(this.getModuleInfo(id)!.moduleSideEffects).toBe(false);
          fn();
        },
        buildEnd() {
          expect(this.getModuleInfo(asset)!.moduleSideEffects).toBe(false);
          fn();
        },
      },
    ],
  },
  afterTest(output) {
    expect(fn).toHaveBeenCalledTimes(2);
    const [chunk] = getOutputChunk(output);
    expect(chunk.code).toContain('main');
    expect(chunk.code).not.toContain('asset');
  },
});
