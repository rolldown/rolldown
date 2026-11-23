import path from 'node:path';
import { defineTest } from 'rolldown-tests';
import { getOutputChunk } from 'rolldown-tests/utils';
import { expect } from 'vitest';

const file = (name: string) => path.join(import.meta.dirname, name);
const resolveFalse = file('resolve-false.js');
const optionFalse = file('option-false.js');
const noHook = file('no-hook.js');
const loadThenTransform = file('load-then-transform.js');
const setterThenNull = file('setter-then-null.js');
const undefinedWrite = file('undefined-write.js');
const packageJsonFalse = file('package-json-false/dep.js');
const resolved: Record<string, { moduleSideEffects: boolean }> = {
  './resolve-false.js': { moduleSideEffects: false },
  './option-false.js': { moduleSideEffects: true },
  './load-then-transform.js': { moduleSideEffects: false },
  './undefined-write.js': { moduleSideEffects: false },
};
const seen: Record<string, Record<string, unknown>> = {};
function record(id: string, step: string, value: unknown) {
  (seen[path.relative(import.meta.dirname, id)] ??= {})[step] = value;
}

// Writing `null` (or `undefined`) sets the value back to "not decided": earlier hook and setter
// values are dropped, and the module is treated as if no hook had given a value. A later hook
// value still wins.
export default defineTest({
  config: {
    treeshake: {
      moduleSideEffects: (id) => (id === optionFalse ? false : undefined),
    },
    plugins: [
      {
        name: 'test-plugin',
        resolveId(source, importer) {
          const options = resolved[source];
          if (options && importer)
            return { id: path.join(path.dirname(importer), source), ...options };
        },
        load(id) {
          if (id !== loadThenTransform) return;
          const info = this.getModuleInfo(id)!;
          info.moduleSideEffects = null;
          record(id, 'load', info.moduleSideEffects);
        },
        transform(code, id) {
          if (id === loadThenTransform) {
            record(id, 'transform', this.getModuleInfo(id)!.moduleSideEffects);
            return { code, moduleSideEffects: false };
          }
          if (id === packageJsonFalse) return { code, moduleSideEffects: true };
        },
        moduleParsed(info) {
          if ([resolveFalse, optionFalse, noHook, packageJsonFalse].includes(info.id)) {
            info.moduleSideEffects = null;
            record(info.id, 'moduleParsed', info.moduleSideEffects);
          } else if (info.id === undefinedWrite) {
            info.moduleSideEffects = undefined as any;
            record(info.id, 'moduleParsed', info.moduleSideEffects);
          } else if (info.id === setterThenNull) {
            info.moduleSideEffects = false;
          }
        },
        buildEnd() {
          this.getModuleInfo(setterThenNull)!.moduleSideEffects = null;
          for (const id of [
            resolveFalse,
            optionFalse,
            noHook,
            loadThenTransform,
            setterThenNull,
            undefinedWrite,
            packageJsonFalse,
          ]) {
            record(id, 'buildEnd', this.getModuleInfo(id)!.moduleSideEffects);
          }
        },
      },
    ],
  },
  afterTest(output) {
    expect(seen).toStrictEqual({
      'resolve-false.js': { moduleParsed: true, buildEnd: true },
      'option-false.js': { moduleParsed: false, buildEnd: false },
      'no-hook.js': { moduleParsed: true, buildEnd: true },
      'load-then-transform.js': { load: true, transform: true, buildEnd: false },
      'setter-then-null.js': { buildEnd: true },
      'undefined-write.js': { moduleParsed: true, buildEnd: true },
      [path.join('package-json-false', 'dep.js')]: { moduleParsed: false, buildEnd: false },
    });
    const [chunk] = getOutputChunk(output);
    expect(chunk.code).toContain('main');
    expect(chunk.code).toContain('RESOLVE_FALSE_EFFECT');
    expect(chunk.code).not.toContain('OPTION_FALSE_EFFECT');
    expect(chunk.code).toContain('NO_HOOK_EFFECT');
    expect(chunk.code).not.toContain('LOAD_THEN_TRANSFORM_EFFECT');
    expect(chunk.code).toContain('SETTER_THEN_NULL_EFFECT');
    expect(chunk.code).toContain('UNDEFINED_WRITE_EFFECT');
    expect(chunk.code).not.toContain('PACKAGE_JSON_EFFECT');
  },
});
