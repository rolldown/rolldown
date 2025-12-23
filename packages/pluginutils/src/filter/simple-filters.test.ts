import picomatch from 'picomatch';
import { describe, expect, test } from 'vitest';
import {
  exactRegex,
  makeIdFiltersToMatchWithQuery,
  prefixRegex,
} from './simple-filters.js';

describe('exactRegex', () => {
  test('supports without flag parameter', () => {
    const regex = exactRegex('foo');
    expect(regex).toStrictEqual(/^foo$/);

    expect(regex.test('foo')).toBe(true);
    expect(regex.test('fooa')).toBe(false);
    expect(regex.test('afoo')).toBe(false);
  });

  test('supports with flag parameter', () => {
    const regex = exactRegex('foo', 'i');
    expect(regex).toStrictEqual(/^foo$/i);

    expect(regex.test('foo')).toBe(true);
    expect(regex.test('Foo')).toBe(true);
    expect(regex.test('Fooa')).toBe(false);
    expect(regex.test('aFoo')).toBe(false);
  });

  test('escapes special characters for Regex', () => {
    const regex = exactRegex('foo(bar)');
    expect(regex).toStrictEqual(/^foo\(bar\)$/);

    expect(regex.test('foo(bar)')).toBe(true);
    expect(regex.test('foo(bar\\)')).toBe(false);
    expect(regex.test('foo(bar)a')).toBe(false);
    expect(regex.test('afoo(bar)')).toBe(false);
  });
});

describe('prefixRegex', () => {
  test('supports without flag parameter', () => {
    const regex = prefixRegex('foo');
    expect(regex).toStrictEqual(/^foo/);

    expect(regex.test('foo')).toBe(true);
    expect(regex.test('fooa')).toBe(true);
    expect(regex.test('afoo')).toBe(false);
  });

  test('supports with flag parameter', () => {
    const regex = prefixRegex('foo', 'i');
    expect(regex).toStrictEqual(/^foo/i);

    expect(regex.test('foo')).toBe(true);
    expect(regex.test('Foo')).toBe(true);
    expect(regex.test('Fooa')).toBe(true);
    expect(regex.test('aFoo')).toBe(false);
  });

  test('escapes special characters for Regex', () => {
    const regex = prefixRegex('foo(bar)');
    expect(regex).toStrictEqual(/^foo\(bar\)/);

    expect(regex.test('foo(bar)')).toBe(true);
    expect(regex.test('foo(bar\\)')).toBe(false);
    expect(regex.test('foo(bar)a')).toBe(true);
    expect(regex.test('afoo(bar)')).toBe(false);
  });
});

describe('makeIdFiltersToMatchWithQuery', () => {
  function expectWithAnyQuery(
    matcher: (path: string) => boolean,
    path: string,
    expected: boolean,
  ) {
    expect(matcher(path), path).toBe(expected);
    expect(matcher(`${path}?foo`), `${path}?foo`).toBe(expected);
    expect(matcher(`${path}?foo=bar`), `${path}?foo=bar`).toBe(expected);
  }

  test('supports glob patterns', () => {
    const input = '/foo/**/*.js';
    const output = makeIdFiltersToMatchWithQuery(input);

    const matcher = picomatch(output);
    expectWithAnyQuery(matcher, '/foo/bar.js', true);
    expectWithAnyQuery(matcher, '/foo/bar.ts', false);
    expect(matcher('/foo/bar.txt?.js')).toBe(true);
  });

  test('supports regex patterns without `$`', () => {
    const input = /\/foo\//;
    const output = makeIdFiltersToMatchWithQuery(input);

    const matcher = (path: string) => output.test(path);
    expectWithAnyQuery(matcher, '/foo/bar.js', true);
    expectWithAnyQuery(matcher, '/bar/bar.ts', false);
    expect(matcher('/foo/bar.txt?.js')).toBe(true);
  });

  test('supports regex patterns with `$`', () => {
    const input = /\/foo\/.*\.js$/;
    const output = makeIdFiltersToMatchWithQuery(input);

    const matcher = (path: string) => output.test(path);
    expectWithAnyQuery(matcher, '/foo/bar.js', true);
    expectWithAnyQuery(matcher, '/foo/bar.ts', false);
    expect(matcher('/foo/bar.txt?.js')).toBe(true);
  });

  test('supports regex patterns with multiple `$`', () => {
    const input = /\/foo\/[^/]*(\/src|\/dist\/[^/]*\.js$|$)/;
    const output = makeIdFiltersToMatchWithQuery(input);

    const matcher = (path: string) => output.test(path);
    expectWithAnyQuery(matcher, '/foo/bar/src/foo', true);
    expectWithAnyQuery(matcher, '/foo/bar/dist/foo.js', true);
    expectWithAnyQuery(matcher, '/foo/bar/dist/foo.ts', false);
    expectWithAnyQuery(matcher, '/foo/bar', true);
    expectWithAnyQuery(matcher, '/foo/bar/', false);
    expect(matcher('/foo/bar/dist/foo.txt?.js')).toBe(true);
  });

  test('supports regex patterns with `\\$`', () => {
    const input = /\/foo\/\$.*\.js$/;
    const output = makeIdFiltersToMatchWithQuery(input);

    const matcher = (path: string) => output.test(path);
    expectWithAnyQuery(matcher, '/foo/$bar.js', true);
    expectWithAnyQuery(matcher, '/foo/$bar.ts', false);
    expect(matcher('/foo/$bar.txt?.js')).toBe(true);
  });
});
