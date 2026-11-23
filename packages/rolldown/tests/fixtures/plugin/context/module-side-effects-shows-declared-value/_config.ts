import path from 'node:path';
import { defineTest } from 'rolldown-tests';
import { getOutputChunk } from 'rolldown-tests/utils';
import { expect } from 'vitest';

const pure = path.join(import.meta.dirname, 'pure.js');
const optionFalse = path.join(import.meta.dirname, 'option-false.js');
const packageJsonFalse = path.join(import.meta.dirname, 'package-json-false/dep.js');
const ids = [pure, optionFalse, packageJsonFalse];
const seen: Record<string, Record<string, unknown>> = {};
function record(hook: string, id: string, read: () => unknown) {
  if (ids.includes(id)) (seen[path.relative(import.meta.dirname, id)] ??= {})[hook] = read();
}

// No hook gives a value, so `getModuleInfo` shows what the options and `package.json` declare,
// else `true`, from `load` on. It is not the result of the statement analysis: `pure.js` has no
// side effects but shows `true`, as in Rollup.
export default defineTest({
  config: {
    treeshake: {
      moduleSideEffects: (id) => (id === optionFalse ? false : undefined),
    },
    plugins: [
      {
        name: 'recorder',
        load(id) {
          record('load', id, () => this.getModuleInfo(id)!.moduleSideEffects);
        },
        transform(_code, id) {
          record('transform', id, () => this.getModuleInfo(id)!.moduleSideEffects);
        },
        moduleParsed(info) {
          record('moduleParsed', info.id, () => info.moduleSideEffects);
        },
        buildEnd() {
          for (const id of ids)
            record('buildEnd', id, () => this.getModuleInfo(id)!.moduleSideEffects);
        },
        renderChunk() {
          for (const id of ids) {
            record('renderChunk', id, () => this.getModuleInfo(id)!.moduleSideEffects);
          }
        },
      },
    ],
  },
  afterTest(output) {
    const everyHook = (value: unknown) => ({
      load: value,
      transform: value,
      moduleParsed: value,
      buildEnd: value,
      renderChunk: value,
    });
    expect(seen).toStrictEqual({
      'pure.js': everyHook(true),
      'option-false.js': everyHook(false),
      [path.join('package-json-false', 'dep.js')]: everyHook(false),
    });
    const [chunk] = getOutputChunk(output);
    expect(chunk.code).toContain('main');
    expect(chunk.code).not.toContain('pure');
    expect(chunk.code).not.toContain('OPTION_FALSE_EFFECT');
    expect(chunk.code).not.toContain('PACKAGE_JSON_EFFECT');
  },
});
