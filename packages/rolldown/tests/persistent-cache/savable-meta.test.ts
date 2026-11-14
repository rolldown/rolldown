import { describe, expect, test } from 'vitest';
import { isEmptyPlainObject, toSavableMetaEntries } from '../../src/plugin/savable-meta';

class Icon {
  name = 'home';
}

describe('toSavableMetaEntries', () => {
  /** The JSON text that is saved for `value` under a key of `meta`. */
  const jsonOf = (value: unknown) => toSavableMetaEntries({ value })?.[0].json;

  test.each([
    ['a string', 'text'],
    ['an empty string', ''],
    ['a number', 1.5],
    ['zero', 0],
    ['a boolean', false],
    ['null', null],
    ['an array', [1, 'two', null, [true]]],
    ['a plain object', { a: { b: [{ c: 'd' }] } }],
    ['two objects with the same content', [{ a: 1 }, { a: 1 }]],
  ])('%s is the same after it is written as JSON and read back', (_name, value) => {
    const json = jsonOf(value);
    expect(json).toBeTypeOf('string');
    expect(JSON.parse(json!)).toStrictEqual(value);
  });

  test('a property with the value `undefined` inside a value is left out', () => {
    expect(jsonOf({ a: undefined, b: { c: undefined, d: 1 } })).toBe('{"b":{"d":1}}');
  });

  test('each top-level key gets one entry, in the order of the keys', () => {
    expect(toSavableMetaEntries({ b: [1], a: 'text', 1: null })).toEqual([
      { key: '1', json: 'null' },
      { key: 'b', json: '[1]' },
      { key: 'a', json: '"text"' },
    ]);
    expect(toSavableMetaEntries({})).toEqual([]);
  });

  test('a top-level key with the value `undefined` gets an entry without JSON text', () => {
    expect(toSavableMetaEntries({ a: 1, cleared: undefined })).toEqual([
      { key: 'a', json: '1' },
      { key: 'cleared' },
    ]);
  });

  const cyclic: Record<string, unknown> = {};
  cyclic.self = { cyclic };
  const sparse: number[] = [];
  sparse[0] = 1;
  sparse[2] = 3;
  const arrayWithProperty = Object.assign([1], { extra: 1 });
  const hidden = Object.defineProperty({}, 'hidden', { value: 1, enumerable: false });
  const shared = { a: 1 };

  test.each([
    ['a function', () => {}],
    ['a symbol', Symbol('s')],
    ['a BigInt', 1n],
    ['NaN', NaN],
    ['Infinity', Infinity],
    ['-Infinity', -Infinity],
    ['-0', -0],
    ['a Map', new Map([[1, 2]])],
    ['a Set', new Set([1])],
    ['a Date', new Date(0)],
    ['a RegExp', /a/],
    ['a Uint8Array', new Uint8Array([1])],
    ['an instance of a class', new Icon()],
    ['a subclass of Array', new (class List extends Array {})()],
    ['an object without a prototype', Object.assign(Object.create(null), { a: 1 })],
    ['an `undefined` in an array', [1, undefined]],
    ['a hole in an array', sparse],
    ['an array with another property', arrayWithProperty],
    ['a property with a symbol key', { [Symbol('s')]: 1 }],
    ['a property that is not enumerable', hidden],
    ['an object that contains itself', cyclic],
    ['the same object two times', [shared, shared]],
    ['a function in an object', { nested: { fn() {} } }],
    ['a Set in an array', [[new Set()]]],
    ['an object with `toJSON`', { toJSON: () => 1 }],
  ])('%s cannot be saved', (_name, value) => {
    expect(toSavableMetaEntries({ value })).toBeUndefined();
    expect(toSavableMetaEntries({ before: 1, nested: { value } })).toBeUndefined();
  });

  test('the same object under two top-level keys cannot be saved', () => {
    expect(toSavableMetaEntries({ a: shared, b: shared })).toBeUndefined();
    expect(toSavableMetaEntries({ a: { ...shared }, b: { ...shared } })).toHaveLength(2);
  });

  test('a `meta` that contains itself cannot be saved', () => {
    const meta: Record<string, unknown> = {};
    meta.self = [meta];
    expect(toSavableMetaEntries(meta)).toBeUndefined();
  });

  test.each([
    ['an instance of a class', new Icon()],
    ['a Map', new Map([['icons', 1]])],
    ['a Map without entries', new Map()],
    ['an array', ['home']],
    ['an object without a prototype', Object.assign(Object.create(null), { a: 1 })],
    ['an object with a symbol key', { a: 1, [Symbol('s')]: 1 }],
    ['an object with a property that is not enumerable', hidden],
    ['a string', 'text'],
    ['a number', 0],
    ['null', null],
    ['undefined', undefined],
  ])('%s as the `meta` itself cannot be saved', (_name, meta) => {
    expect(toSavableMetaEntries(meta)).toBeUndefined();
  });

  test('one half of a surrogate pair can be saved in a value, but not in a top-level key', () => {
    const value = { text: '\ud800', '\ud800': 1 };
    expect(JSON.parse(jsonOf(value)!)).toStrictEqual(value);
    expect(toSavableMetaEntries({ '\ud800x': 1 })).toBeUndefined();
    expect(toSavableMetaEntries({ '\ud83d\ude00': 1 })).toEqual([
      { key: '\ud83d\ude00', json: '1' },
    ]);
  });

  test('a value that is nested too deeply cannot be saved', () => {
    let deep: unknown[] = [];
    for (let i = 0; i < 100_000; i++) deep = [deep];
    expect(toSavableMetaEntries({ deep })).toBeUndefined();
  });
});

describe('isEmptyPlainObject', () => {
  test('only a plain object without properties is empty', () => {
    expect(isEmptyPlainObject({})).toBe(true);
    for (const meta of [
      { a: undefined },
      { [Symbol('s')]: 1 },
      Object.defineProperty({}, 'hidden', { value: 1, enumerable: false }),
      Object.create(null),
      new Map(),
      new Icon(),
      [],
      '',
      0,
      null,
      undefined,
    ]) {
      expect(isEmptyPlainObject(meta)).toBe(false);
    }
  });
});
