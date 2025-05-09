import { describe, expectTypeOf, test } from 'vitest';
import { makeIdFiltersToMatchWithQuery } from './simple-filters';

describe('makeIdFiltersToMatchWithQuery', () => {
  test('single string input', () => {
    const input = 'foo';
    expectTypeOf(makeIdFiltersToMatchWithQuery(input)).toEqualTypeOf<string>();

    // string literal should return normal string
    expectTypeOf(makeIdFiltersToMatchWithQuery('foo')).not.toEqualTypeOf<
      'foo'
    >();
    expectTypeOf(makeIdFiltersToMatchWithQuery('foo')).toEqualTypeOf<string>();
  });

  test('single regex input', () => {
    expectTypeOf(makeIdFiltersToMatchWithQuery(/foo/)).toEqualTypeOf<RegExp>();
  });

  test('single string or regex input', () => {
    const input = 'foo' as string | RegExp;
    expectTypeOf(makeIdFiltersToMatchWithQuery(input)).toEqualTypeOf<
      string | RegExp
    >();
  });

  test('array string input', () => {
    const input = ['foo'];
    expectTypeOf(makeIdFiltersToMatchWithQuery(input)).toEqualTypeOf<
      string[]
    >();

    // string literal should return normal string
    expectTypeOf(makeIdFiltersToMatchWithQuery(['foo'])).not.toEqualTypeOf<
      'foo'[]
    >();
    expectTypeOf(makeIdFiltersToMatchWithQuery(['foo'])).toEqualTypeOf<
      string[]
    >();
  });

  test('array regex input', () => {
    expectTypeOf(makeIdFiltersToMatchWithQuery([/foo/])).toEqualTypeOf<
      RegExp[]
    >();
  });

  test('array string or regex input', () => {
    const input = ['foo'] as (string | RegExp)[];
    expectTypeOf(makeIdFiltersToMatchWithQuery(input)).toEqualTypeOf<
      (string | RegExp)[]
    >();
  });

  test('mixed input', () => {
    const input = ['foo', /bar/] as (string | RegExp)[] | string | RegExp;
    expectTypeOf(makeIdFiltersToMatchWithQuery(input)).toEqualTypeOf<
      (string | RegExp)[] | string | RegExp
    >();
  });
});
