import { execa } from 'execa';
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

  it('resolves runtime relative dynamic imports in a deferred config function against the config directory', async () => {
    // Runs in a plain Node child process: the environment the CLI actually loads
    // configs in, and vite-node would intercept the emitted entry's dynamic
    // import. The CLI invokes a deferred config function only after `loadConfig`
    // has already returned and cleaned up its bundling artifacts.
    const script = [
      `import { loadConfig } from 'rolldown/config';`,
      `const config = await loadConfig(${JSON.stringify(
        path.join(fixtures, 'deferred-dynamic-import.config.ts'),
      )});`,
      `if (typeof config !== 'function') throw new TypeError('expected bundled config function');`,
      `const resolved = await config({});`,
      `if (resolved.input !== './deferred-entry.js') {`,
      `  throw new Error('unexpected deferred config: ' + JSON.stringify(resolved));`,
      `}`,
      `console.log('deferred-config-ok');`,
    ].join('\n');
    const ret = await execa('node', ['--input-type=module', '--eval', script], {
      cwd: import.meta.dirname,
    });
    expect(ret.exitCode).toBe(0);
    expect(ret.stdout).toContain('deferred-config-ok');
  });
});
