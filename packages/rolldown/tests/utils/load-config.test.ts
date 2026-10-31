import { readdir } from 'node:fs/promises';
import path from 'node:path';
import { loadConfig } from 'rolldown/config';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';

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
});

describe('loadConfig bundle configLoader', () => {
  let filesBefore: string[];

  beforeEach(async () => {
    filesBefore = await readdir(fixtures);
  });

  afterEach(async () => {
    expect(await readdir(fixtures)).toStrictEqual(filesBefore);
  });

  it('keeps dynamic imports available to a config function called after loading', async () => {
    const config = await loadConfig(path.join(fixtures, 'dynamic-function.config.ts'));

    expect(config).toBeTypeOf('function');
    await expect((config as () => Promise<unknown>)()).resolves.toStrictEqual({
      input: './dynamic-entry.js',
    });
  });

  it('resolves runtime-relative requires from the config directory', async () => {
    const config = await loadConfig(path.join(fixtures, 'runtime-require.config.cts'));

    expect(config).toStrictEqual({ input: path.join(fixtures, 'native.config.mjs') });
  });

  it('rejects when the config throws `undefined`', async () => {
    const error = await loadConfig(path.join(fixtures, 'throw-undefined.config.ts')).catch(
      (error: unknown) => error,
    );

    expect(error).toBeInstanceOf(Error);
    expect((error as Error).message).toMatch(/Error happened while loading config/);
    expect((error as Error).cause).toBeUndefined();
  });

  it('evaluates each load of the same config separately', async () => {
    const configFile = path.join(fixtures, 'runtime-require.config.cts');

    const [first, second] = await Promise.all([loadConfig(configFile), loadConfig(configFile)]);

    expect(first).toStrictEqual(second);
    expect(first).not.toBe(second);
  });
});
