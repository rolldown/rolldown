import path from 'node:path';
import { defineTest } from 'rolldown-tests';
import { getOutputChunk } from 'rolldown-tests/utils';
import { expect } from 'vitest';

const drop = path.join(import.meta.dirname, 'drop.js');
const keep = path.join(import.meta.dirname, 'keep.js');
const outputHook = path.join(import.meta.dirname, 'output-hook.js');

// Writes made in `buildEnd` reach tree shaking in both directions. Rolldown links the modules
// before the output hooks run, so a write in `renderStart` changes nothing.
export default defineTest({
  config: {
    plugins: [
      {
        name: 'test-plugin',
        resolveId(source) {
          if (source === './keep.js') return { id: keep, moduleSideEffects: false };
        },
        buildEnd() {
          this.getModuleInfo(drop)!.moduleSideEffects = false;
          this.getModuleInfo(keep)!.moduleSideEffects = true;
        },
        renderStart() {
          this.getModuleInfo(outputHook)!.moduleSideEffects = false;
        },
      },
    ],
  },
  afterTest(output) {
    const [chunk] = getOutputChunk(output);
    expect(chunk.code).not.toContain('DROP_EFFECT');
    expect(chunk.code).toContain('KEEP_EFFECT');
    expect(chunk.code).toContain('OUTPUT_HOOK_EFFECT');
  },
});
