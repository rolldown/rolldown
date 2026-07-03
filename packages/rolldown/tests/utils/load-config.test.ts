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
});
