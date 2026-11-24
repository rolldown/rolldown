import path from 'node:path';
import { defineTest } from 'rolldown-tests';
import { expect, vi } from 'vitest';

const asset = path.join(import.meta.dirname, 'asset.txt');
const fn = vi.fn();

export default defineTest({
  config: {
    // The native asset plugin loads `asset.txt` with `moduleSideEffects: false`.
    moduleTypes: { '.txt': 'asset' },
    plugins: [
      {
        name: 'test-plugin',
        transform(_code, id) {
          if (id !== asset) return;
          expect(this.getModuleInfo(id)!.moduleSideEffects).toBe(false);
          fn();
        },
      },
    ],
  },
  afterTest() {
    expect(fn).toHaveBeenCalledTimes(1);
  },
});
