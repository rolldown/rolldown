import path from 'node:path';
import { defineTest } from 'rolldown-tests';
import { isolatedDeclarationPlugin } from 'rolldown/experimental';
import { expect, vi } from 'vitest';

const types = path.join(import.meta.dirname, 'types.ts');
const fn = vi.fn();

export default defineTest({
  config: {
    input: 'main.ts',
    plugins: [
      // The plugin resolves the type-only import with the hook below and loads that resolution.
      isolatedDeclarationPlugin(),
      {
        name: 'test-plugin',
        resolveId(source, importer) {
          if (source === './types' && importer) {
            return { id: types, meta: { fromHook: true }, moduleSideEffects: false };
          }
        },
        buildEnd() {
          const info = this.getModuleInfo(types)!;
          expect(info.meta).toStrictEqual({ fromHook: true });
          expect(info.moduleSideEffects).toBe(false);
          fn();
        },
      },
    ],
  },
  afterTest() {
    expect(fn).toHaveBeenCalledTimes(1);
  },
});
