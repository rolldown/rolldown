import fs from 'node:fs';
import path from 'node:path';
import vm from 'node:vm';
import { beforeEach, describe, expect, test } from 'vitest';

// The registry that `experimentalInlineCommonChunks` adds to the runtime module:
// `__share(id, factory)` (first registration wins) and `__share_require(id)` with its five states:
// unregistered, registered, executing, completed, failed. Loaded straight from `runtime-base.js`
// so the tests pin the helper text the bundler ships, not a copy.

const runtimeSource = fs.readFileSync(
  path.resolve(import.meta.dirname, '../../../../../crates/rolldown/src/runtime/runtime-base.js'),
  'utf8',
);

interface Registry {
  __share: (id: string, factory: (exports: Record<string, unknown>) => void) => void;
  __share_require: (id: string) => Record<string, unknown>;
  __share_export: (target: object, all: Record<string, () => unknown>) => void;
}

function loadRegistry(): Registry {
  const body = runtimeSource.replace(/^export var /gm, 'var ');
  return vm.runInNewContext(
    `${body}\n({ __share, __share_require, __share_export })`,
    {},
  ) as Registry;
}

describe('runtime registry', () => {
  let registry: Registry;
  beforeEach(() => {
    registry = loadRegistry();
  });

  test('requiring an unregistered id throws a descriptive error', () => {
    expect(() => registry.__share_require('nope')).toThrow(
      'Shared chunk "nope" was not registered before it was required.',
    );
  });

  test('the first registration wins and the factory runs once', () => {
    let runs = 0;
    registry.__share('r', (exports) => {
      runs += 1;
      registry.__share_export(exports, { which: () => 'first' });
    });
    registry.__share('r', (exports) => {
      runs += 1;
      registry.__share_export(exports, { which: () => 'second' });
    });
    const first = registry.__share_require('r');
    expect(first.which).toBe('first');
    expect(registry.__share_require('r')).toBe(first);
    expect(runs).toBe(1);
  });

  test('exports are live getters', () => {
    let value = 1;
    registry.__share('live', (exports) => {
      registry.__share_export(exports, { value: () => value });
    });
    const exports = registry.__share_require('live');
    expect(exports.value).toBe(1);
    value = 2;
    expect(exports.value).toBe(2);
    expect(Object.keys(exports)).toEqual(['value']);
  });

  test('an executing record hands out the same partial exports object', () => {
    let seenDuringExecution: Record<string, unknown> | undefined;
    registry.__share('cycle', (exports) => {
      seenDuringExecution = registry.__share_require('cycle');
      registry.__share_export(exports, { done: () => true });
    });
    const exports = registry.__share_require('cycle');
    expect(seenDuringExecution).toBe(exports);
    expect(exports.done).toBe(true);
  });

  test.each<[string, unknown]>([
    ['an Error', new Error('factory failed')],
    ['undefined', undefined],
    ['null', null],
    ['a string', 'text'],
  ])('a factory throwing %s rethrows the same value on every require', (_label, thrown) => {
    let runs = 0;
    registry.__share('bad', () => {
      runs += 1;
      throw thrown;
    });
    for (let i = 0; i < 2; i += 1) {
      let caught: unknown = 'not thrown';
      try {
        registry.__share_require('bad');
      } catch (error) {
        caught = error;
      }
      expect(caught).toBe(thrown);
    }
    expect(runs).toBe(1);
  });
});
