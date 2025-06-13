import { describe, expect, test } from 'vitest';
import { Platform, tryFrom } from '../src/inner_bundler_options/types/platform';
import { get_wasi_target_triple } from '../src/wasi_features';

describe('Platform string conversion', () => {
  test.each([
    { input: 'node', expected: Platform.Node, expectedName: 'Node' },
    { input: 'browser', expected: Platform.Browser, expectedName: 'Browser' },
    { input: 'neutral', expected: Platform.Neutral, expectedName: 'Neutral' },
    { input: 'wasi', expected: Platform.Wasi, expectedName: 'Wasi' },
    { input: 'wasip1', expected: Platform.Wasi, expectedName: 'Wasi' },
    { input: 'wasip2', expected: Platform.WasiP2, expectedName: 'WasiP2' },
  ])('converts $input to $expectedName', ({ input, expected }) => {
    const platform = tryFrom(input);
    expect(platform).toBe(expected);
  });

  test('throws for invalid platform string', () => {
    expect(() => tryFrom('invalid')).toThrow();
  });
});

describe('WASI target triples', () => {
  test('returns correct target triple for WASI Preview 1', () => {
    expect(get_wasi_target_triple(Platform.Wasi)).toBe('wasm32-wasip1-threads');
  });

  test('returns correct target triple for WASI Preview 2', () => {
    expect(get_wasi_target_triple(Platform.WasiP2)).toBe('wasm32-wasip2');
  });

  test('returns null for non-WASI platforms', () => {
    expect(get_wasi_target_triple(Platform.Node)).toBeNull();
  });
});
