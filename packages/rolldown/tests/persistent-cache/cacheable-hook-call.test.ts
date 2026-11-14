import { describe, expect, test } from 'vitest';
import type { BindingPluginContext } from '../../src/binding.cjs';
import { callCacheableHook } from '../../src/plugin/cacheable-hook-call';
import type { PluginContextData, RecordedHookCall } from '../../src/plugin/plugin-context-data';

describe('callCacheableHook without the persistent cache', () => {
  // Without the cache there is no `access`, and nothing else is used.
  const call = <T>(handler: () => T) =>
    callCacheableHook(
      {} as PluginContextData,
      {} as BindingPluginContext,
      undefined,
      {} as RecordedHookCall,
      handler,
    );

  test('an async handler gives the same promise object', () => {
    const promise = Promise.resolve({ id: 'a' });
    expect(call(() => promise)).toBe(promise);
  });

  test('a sync handler gives the same plain value', () => {
    const value = { id: 'a' };
    expect(call(() => value)).toBe(value);
    expect(call(() => null)).toBe(null);
    expect(call(() => undefined)).toBe(undefined);
  });

  test('an error of a sync handler is thrown at once', () => {
    const error = new Error('fail');
    expect(() =>
      call(() => {
        throw error;
      }),
    ).toThrow(error);
  });
});
