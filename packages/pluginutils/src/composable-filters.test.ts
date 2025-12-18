import { describe, expect, test } from 'vitest';
import type { QueryFilterObject } from './composable-filters';
import {
  and,
  exclude,
  id,
  importerId,
  include,
  interpreter,
  or,
  queries,
  query,
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

describe('importerIdFilter', () => {
  test('string pattern', () => {
    // exact match
    expect(
      interpreter(
        [include(importerId('/src/main.ts'))],
        undefined,
        '/src/foo.ts',
        undefined,
        '/src/main.ts',
      ),
    ).toBe(true);

    // no match
    expect(
      interpreter(
        [include(importerId('/src/main.ts'))],
        undefined,
        '/src/foo.ts',
        undefined,
        '/src/other.ts',
      ),
    ).toBe(false);
  });

  test('regex pattern', () => {
    // regex match
    expect(
      interpreter(
        [include(importerId(/\.vue$/))],
        undefined,
        '/src/component.ts',
        undefined,
        '/src/App.vue',
      ),
    ).toBe(true);

    // regex no match
    expect(
      interpreter(
        [include(importerId(/\.vue$/))],
        undefined,
        '/src/component.ts',
        undefined,
        '/src/main.ts',
      ),
    ).toBe(false);
  });

  test('cleanUrl option', () => {
    // with cleanUrl, should strip query params
    expect(
      interpreter(
        [include(importerId('/src/main.ts', { cleanUrl: true }))],
        undefined,
        '/src/foo.ts',
        undefined,
        '/src/main.ts?query=1',
      ),
    ).toBe(true);

    // without cleanUrl, query params are included
    expect(
      interpreter(
        [include(importerId('/src/main.ts'))],
        undefined,
        '/src/foo.ts',
        undefined,
        '/src/main.ts?query=1',
      ),
    ).toBe(false);
  });

  test('combined with id filter', () => {
    // both id and importerId must match
    expect(
      interpreter(
        [include(and(id(/\.ts$/), importerId(/\.vue$/)))],
        undefined,
        '/src/component.ts',
        undefined,
        '/src/App.vue',
      ),
    ).toBe(true);

    // id matches but importerId doesn't
    expect(
      interpreter(
        [include(and(id(/\.ts$/), importerId(/\.vue$/)))],
        undefined,
        '/src/component.ts',
        undefined,
        '/src/main.ts',
      ),
    ).toBe(false);
  });

  test('exclude with importerId', () => {
    // exclude files imported from node_modules
    expect(
      interpreter(
        [exclude(importerId(/node_modules/))],
        undefined,
        '/src/foo.ts',
        undefined,
        '/node_modules/some-package/index.js',
      ),
    ).toBe(false);

    // include files imported from src
    expect(
      interpreter(
        [exclude(importerId(/node_modules/))],
        undefined,
        '/src/foo.ts',
        undefined,
        '/src/utils.ts',
      ),
    ).toBe(true);
  });

  test('returns false when importerId is undefined', () => {
    expect(
      interpreter(
        [include(importerId('/src/main.ts'))],
        undefined,
        '/src/foo.ts',
        undefined,
        undefined,
      ),
    ).toBe(false);
  });
});
