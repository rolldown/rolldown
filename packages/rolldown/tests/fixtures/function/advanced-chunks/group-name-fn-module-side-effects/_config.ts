import path from 'node:path';
import { defineTest } from 'rolldown-tests';
import { expect } from 'vitest';

const effect = path.join(import.meta.dirname, 'effect.js');
const optionFalse = path.join(import.meta.dirname, 'option-false.js');
const seen: Record<string, unknown> = {};

// The chunking context shows the same value as the plugin context.
export default defineTest({
  config: {
    treeshake: {
      moduleSideEffects: (id) => (id === optionFalse ? false : undefined),
    },
    output: {
      codeSplitting: {
        groups: [
          {
            name(_id, ctx) {
              seen.effect = ctx.getModuleInfo(effect)!.moduleSideEffects;
              seen.optionFalse = ctx.getModuleInfo(optionFalse)!.moduleSideEffects;
              return null;
            },
          },
        ],
      },
    },
  },
  afterTest() {
    expect(seen).toStrictEqual({ effect: true, optionFalse: false });
  },
});
