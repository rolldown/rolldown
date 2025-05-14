// cSpell:ignore fooa, afoo
import { describe, expect, test } from 'vitest';
import {
  and,
  exclude,
  include,
  interpreter,
  or,
  queries,
  query,
  QueryFilterObject,
} from './composable-filters';

function queryFilter(
  id: string,
  queryFilterObject: QueryFilterObject,
): boolean {
  let topLevelFilterExpression = include(
    queries(queryFilterObject),
  );
  return interpreter([topLevelFilterExpression], undefined, id, undefined);
}
describe('queryFilter', () => {
  test('boolean', () => {
    expect(
      queryFilter('/foo/bar?a=1111&b=2222', {
        a: true,
        b: true,
      }),
    ).toBe(true);

    expect(
      queryFilter('/foo/bar?a=1111&b=2222', {
        a: true,
        b: false,
      }),
    ).toBe(false);

    expect(
      queryFilter('/foo/bar?a=1111&b=2222', {
        a: true,
      }),
    ).toBe(true);

    expect(
      queryFilter('/foo/bar?a=1111&b=2222', {
        bar: false,
      }),
    ).toBe(true);

    expect(
      queryFilter('/foo/bar?a', {
        a: true,
      }),
    ).toBe(true);

    expect(
      queryFilter('/foo/bar?a=', {
        a: true,
      }),
    ).toBe(true);
  });

  test('string', () => {
    expect(
      queryFilter('/foo/bar?a=1111&b=2222', {
        a: '1111',
      }),
    ).toBe(true);

    expect(
      queryFilter('/foo/bar?a=1111&b=bar', {
        a: '1111',
        b: 'bar',
      }),
    ).toBe(true);

    expect(
      queryFilter('/foo/bar?a=1111&b=bar', {
        d: '1111',
      }),
    ).toBe(false);
  });

  test('regex', () => {
    expect(
      queryFilter('/foo/bar?a=1111&b=2222', {
        a: /[\d]+/,
      }),
    ).toBe(true);

    expect(
      queryFilter('/foo/bar?a=1111&b=bar', {
        b: /bar/,
      }),
    ).toBe(true);

    expect(
      queryFilter('/foo/bar?a=1111&b=bar', {
        d: /1111/,
      }),
    ).toBe(false);
  });

  test('custom', () => {
    // https://github.com/sveltejs/vite-plugin-svelte/blob/3589433cd19464c484f560516d41e670e5d40710/packages/vite-plugin-svelte/src/utils/id.js#L35-L40
    let filterExpr = or(
      query('url', true),
      and(
        query('svelte', false),
        or(
          query('raw', true),
          query('direct', true),
        ),
      ),
    );
    // include `url`, should return `false`
    expect(interpreter(
      [exclude(
        filterExpr,
      )],
      undefined,
      '/foo/bar?url=1',
      undefined,
    )).toBe(false);
    // don't have `svelte` and has `raw`, should return `false`
    expect(interpreter(
      [exclude(
        filterExpr,
      )],
      undefined,
      '/foo/bar?raw=1',
      undefined,
    )).toBe(false);
    // don't have `svelte`, but don't have `raw` and `direct` neither, should return `true`
    expect(interpreter(
      [exclude(
        filterExpr,
      )],
      undefined,
      '/foo/bar',
      undefined,
    )).toBe(true);
    // have `url` should return `false` even query also has `svelte`
    expect(interpreter(
      [exclude(
        filterExpr,
      )],
      undefined,
      '/foo/bar?url=1111&svelte=true',
      undefined,
    )).toBe(false);
  });
});
