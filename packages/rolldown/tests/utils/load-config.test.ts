import { readdir } from 'node:fs/promises';
import path from 'node:path';
import { loadConfig } from 'rolldown/config';
import { describe, expect, it } from 'vitest';

const fixtures = path.join(import.meta.dirname, 'fixtures', 'load-config');

describe('loadConfig native configLoader', () => {
  it('loads an mjs config via the native loader', async () => {
    const config = await loadConfig(path.join(fixtures, 'native.config.mjs'), {
      configLoader: 'native',
    });
    expect(config).toStrictEqual({ input: './entry.js' });
  });

  it('wraps native load failures with a helpful hint and preserves the cause', async () => {
    await expect(
      loadConfig(path.join(fixtures, 'throws.config.mjs'), {
        configLoader: 'native',
      }),
    ).rejects.toThrow(/native.*config loader/i);

    try {
      await loadConfig(path.join(fixtures, 'throws.config.mjs'), {
        configLoader: 'native',
      });
      expect.unreachable();
    } catch (err) {
      const cause = (err as { cause?: Error }).cause;
      expect(cause?.message).toContain('boom from config');
    }
  });

  it('defaults to the bundle loader when no option is passed', async () => {
    const config = await loadConfig(path.join(fixtures, 'native.config.mjs'));
    expect(config).toStrictEqual({ input: './entry.js' });
  });

  it('keeps bundled dynamic imports available to a deferred config function', async () => {
    const config = await loadConfig(path.join(fixtures, 'dynamic-function.config.ts'));
    if (typeof config !== 'function') {
      throw new TypeError('expected bundled config function');
    }
    await expect(config({})).resolves.toStrictEqual({ input: './dynamic-entry.js' });
  });
});

describe('loadConfig bundle configLoader', () => {
  it('keeps runtime-relative resolution working from the config directory', async () => {
    const config = await loadConfig(path.join(fixtures, 'runtime-require.config.cts'));

    expect(config).toStrictEqual({
      input: path.join(fixtures, 'runtime-required-entry.js'),
    });
  });

  it('rejects when the bundled config rejects with `undefined`', async () => {
    const error = await loadConfig(path.join(fixtures, 'throw-undefined.config.ts')).catch(
      (error: unknown) => error,
    );

    expect(error).toBeInstanceOf(Error);
    expect((error as Error).message).toMatch(/Error happened while loading config/);
    expect((error as Error & { cause?: unknown }).cause).toBeUndefined();
  });

  it('leaves no generated file behind, whether the config loads or fails', async () => {
    const before = (await readdir(fixtures)).sort();

    await loadConfig(path.join(fixtures, 'runtime-require.config.cts'));
    await loadConfig(path.join(fixtures, 'throw-undefined.config.ts')).catch(() => {});

    expect((await readdir(fixtures)).sort()).toStrictEqual(before);
  });
});
