import { describe, expect, test } from 'vitest';
import { Platform } from '../src/inner_bundler_options/types/platform';
import { is_wasi_platform, is_wasi_preview2 } from '../src/wasi_features';

describe('WASI platform detection', () => {
  // Define platforms for testing
  const platforms = {
    Node: Platform.Node,
    Browser: Platform.Browser,
    Neutral: Platform.Neutral,
    Wasi: Platform.Wasi,
    WasiP2: Platform.WasiP2,
  };

  describe('is_wasi_platform function', () => {
    test.each([
      { platform: 'Node', expected: false },
      { platform: 'Browser', expected: false },
      { platform: 'Neutral', expected: false },
      { platform: 'Wasi', expected: true },
      { platform: 'WasiP2', expected: true },
    ])(
      'correctly detects $platform as WASI: $expected',
      ({ platform, expected }) => {
        expect(is_wasi_platform(platforms[platform as keyof typeof platforms]))
          .toBe(expected);
      },
    );
  });

  describe('is_wasi_preview2 function', () => {
    test.each([
      { platform: 'Node', expected: false },
      { platform: 'Browser', expected: false },
      { platform: 'Neutral', expected: false },
      { platform: 'Wasi', expected: false },
      { platform: 'WasiP2', expected: true },
    ])(
      'correctly detects $platform as WASI Preview 2: $expected',
      ({ platform, expected }) => {
        expect(is_wasi_preview2(platforms[platform as keyof typeof platforms]))
          .toBe(expected);
      },
    );
  });
});
