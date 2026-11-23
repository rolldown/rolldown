import type { Plugin } from 'rolldown';
import { rolldown } from 'rolldown';
import { isWasiTest, sleep } from 'rolldown-tests/utils';
import { describe, expect, test, vi } from 'vitest';
import { aggregateBindingErrorsIntoJsError } from '../../src/utils/error';

async function buildWithPlugin(plugin: Plugin) {
  try {
    const build = await rolldown({
      input: './main.js',
      cwd: import.meta.dirname,
      plugins: [plugin],
    });
    await build.write({});
  } catch (e) {
    return e as Error;
  }
}

describe('Plugin renderError hook', () => {
  test('calls renderError with the rendering error', async () => {
    const renderErrorFn = vi.fn();
    const renderChunkFn = vi.fn();
    const error = await buildWithPlugin({
      name: 'test',
      renderStart() {
        renderChunkFn();
        throw new Error('renderStart error');
      },
      renderError: (error) => {
        renderErrorFn();
        expect(error!.message).toContain('renderStart error');
      },
    });
    expect(error!.message).toContain('renderStart error');
    expect(renderErrorFn).toHaveBeenCalledTimes(1);
  });

  test('preserves rendering and renderError failures', async () => {
    const error = await buildWithPlugin({
      name: 'test',
      renderStart() {
        this.error({ code: 'PRIMARY_FAILURE', message: 'rendering failed' });
      },
      renderError() {
        throw new Error('cleanup failed');
      },
    });
    expect(error).toMatchObject({
      message: expect.stringMatching(/rendering failed[\s\S]*cleanup failed/),
      errors: [{ message: 'rendering failed' }, { message: 'cleanup failed' }],
    });
  });
});

test('Plugin renderError hook waits for an async handler', async () => {
  let finished = false;
  await buildWithPlugin({
    name: 'test',
    renderStart() {
      throw new Error('renderStart error');
    },
    async renderError() {
      await sleep(50);
      finished = true;
    },
  });
  expect(finished).toBe(true);
});

test('Plugin renderError hook reports an async rejection', async () => {
  const error = await buildWithPlugin({
    name: 'test',
    renderStart() {
      throw new Error('renderStart error');
    },
    async renderError() {
      await Promise.resolve();
      throw new Error('renderError error');
    },
  });
  expect(error!.message).toContain('renderError error');
});

test('Plugin renderStart hook reports an async rejection', async () => {
  const error = await buildWithPlugin({
    name: 'test',
    async renderStart() {
      await Promise.resolve();
      throw new Error('renderStart error');
    },
  });
  expect(error!.message).toContain('renderStart error');
});

describe('Plugin buildEnd hook', async () => {
  test('call buildEnd hook with error', async () => {
    const buildEndFn = vi.fn();
    const error = await buildWithPlugin({
      name: 'test',
      buildStart() {
        throw new Error('buildStart error');
      },
      buildEnd: (error) => {
        buildEndFn();
        expect(error!.message).toContain('buildStart error');
      },
    });
    expect(error!.message).toContain('buildStart error');
    expect(buildEndFn).toHaveBeenCalledTimes(1);
  });

  test('call buildEnd hook without error', async () => {
    const buildEndFn = vi.fn();
    const error = await buildWithPlugin({
      name: 'test',
      buildEnd: (error) => {
        buildEndFn();
        expect(error).toBeUndefined();
      },
    });
    expect(error).toBeUndefined();
    expect(buildEndFn).toHaveBeenCalledTimes(1);
  });

  test('preserves the build failure when buildEnd also fails', async () => {
    const buildEndFn = vi.fn();
    const error = await buildWithPlugin({
      name: 'test',
      buildStart() {
        this.error('build failed');
      },
      async buildEnd(error) {
        buildEndFn(error?.message);
        throw new Error('buildEnd failed');
      },
    });

    expect(buildEndFn).toHaveBeenCalledExactlyOnceWith(expect.stringContaining('build failed'));
    expect(error).toMatchObject({
      message: expect.stringMatching(/build failed[\s\S]*buildEnd failed/),
      errors: [{ message: 'build failed' }, { message: 'buildEnd failed' }],
    });
  });
});

describe('Plugin closeBundle hook', async () => {
  test('call closeBundle hook if has error', async () => {
    const closeBundleFn = vi.fn();
    const error = await buildWithPlugin({
      name: 'test',
      load() {
        throw new Error('load error');
      },
      closeBundle: () => {
        closeBundleFn();
      },
    });
    expect(error!.message).toContain('load error');
    expect(closeBundleFn).toHaveBeenCalledTimes(1);
  });

  test('call closeBundle hook with error argument when build fails', async () => {
    let receivedError: Error | undefined;
    const error = await buildWithPlugin({
      name: 'test',
      load() {
        throw new Error('load error');
      },
      closeBundle(error) {
        receivedError = error;
      },
    });
    expect(error!.message).toContain('load error');
    expect(receivedError).toBeDefined();
    expect(receivedError!.message).toContain('load error');
  });

  test('call closeBundle hook without error argument when build succeeds', async () => {
    let receivedError: Error | undefined = new Error('should be cleared');
    const build = await rolldown({
      input: './main.js',
      cwd: import.meta.dirname,
      plugins: [
        {
          name: 'test',
          closeBundle(error) {
            receivedError = error;
          },
        },
      ],
    });
    await build.generate();
    await build.close();
    expect(receivedError).toBeUndefined();
  });

  test('call closeBundle with bundle close', async () => {
    const closeBundleFn = vi.fn();
    const build = await rolldown({
      input: './main.js',
      cwd: import.meta.dirname,
      plugins: [
        {
          name: 'test',
          closeBundle: () => {
            closeBundleFn();
          },
        },
      ],
    });
    await build.generate();
    await build.close();
    expect(closeBundleFn).toHaveBeenCalledTimes(1);
  });

  test('should error at generate if bundle already closed', async () => {
    try {
      const build = await rolldown({
        input: './main.js',
        cwd: import.meta.dirname,
      });
      await build.close();
      await build.write();
    } catch (error: any) {
      expect(error.message).toMatchInlineSnapshot(
        `
        "[ALREADY_CLOSED] Bundle is already closed, no more calls to "generate" or "write" are allowed.
        "
      `,
      );
    }
  });
});

test('call transformContext error', async () => {
  const error = await buildWithPlugin({
    name: 'test',
    transform() {
      this.error('transform hook error');
    },
  });
  expect(error!.message).toContain('transform hook error');
});

// Under the wasm binding the plugin error is reconstructed as a plain `Error`, losing its `url`.
test.skipIf(isWasiTest)('includes the url of a plugin error in the message', async () => {
  const error: any = await buildWithPlugin({
    name: 'test',
    transform() {
      this.error({ message: 'transform hook error', url: 'https://example.com/docs' });
    },
  });
  expect(error.message).toContain('transform hook error\nhttps://example.com/docs');
  // Same as Rollup, the error object itself keeps the url out of its message.
  expect(error.errors[0].message).toBe('transform hook error');
  expect(error.errors[0].url).toBe('https://example.com/docs');
});

test('prints the url of a plugin warning', async () => {
  const warn = vi.spyOn(console, 'warn').mockImplementation(() => {});
  try {
    const build = await rolldown({
      input: './main.js',
      cwd: import.meta.dirname,
      plugins: [
        {
          name: 'test',
          transform() {
            this.warn({ message: 'transform hook warning', url: 'https://example.com/docs' });
          },
        },
      ],
    });
    await build.generate();
    await build.close();
    expect(warn).toHaveBeenCalledWith(
      '(test plugin) transform hook warning\nhttps://example.com/docs',
    );
  } finally {
    warn.mockRestore();
  }
});

// #4141
// Under the wasm binding the `structuredClone` failure degrades to a plain `Error`, losing the `DataCloneError` name.
test.skipIf(isWasiTest)('should print original error if it can not be assigned', async () => {
  const error = await buildWithPlugin({
    name: 'test',
    transform() {
      const proxy = new Proxy({ a: 1 }, {});
      structuredClone(proxy);
    },
  });
  expect(error!.message).toContain('DataCloneError: #<Object> could not be cloned');
});

describe('Error output format', () => {
  test('should correctly output the custom error defined on the rust side', async () => {
    try {
      const build = await rolldown({
        input: './error.js',
        cwd: import.meta.dirname,
      });
      await build.write();
    } catch (error: any) {
      expect(removeAnsiColors(error.message)).toMatchSnapshot();
    }
  });

  test('bundler initialize error occurs', async () => {
    try {
      const build = await rolldown({
        input: './main.js',
        cwd: import.meta.dirname,
        transform: {
          target: 'es5',
        },
      });
      await build.write({});
    } catch (error: any) {
      expect(removeAnsiColors(error.message)).toMatchSnapshot();
    }
  });

  // Output-option callbacks skip the plugin hook normalizer, so a thrown
  // `undefined`/`null` reaches the summary formatter verbatim and must not make it throw.
  test.each([
    ['undefined', undefined],
    ['null', null],
  ])('summarizes a %s thrown by an output-option callback', async (label, thrown) => {
    // WASI reconstructs callback failures as Errors, so exercise the nullish formatter directly.
    const formatted = aggregateBindingErrorsIntoJsError([
      { type: 'JsError', field0: thrown as unknown as Error },
    ]);
    expect(formatted.message).toContain(`Error: ${label}`);

    const build = await rolldown({
      input: './main.js',
      cwd: import.meta.dirname,
    });
    try {
      const error: any = await build
        .generate({
          entryFileNames: () => {
            throw thrown;
          },
        })
        .then(
          () => undefined,
          (e: unknown) => e,
        );
      expect(error).toBeInstanceOf(Error);
      expect(error.message).not.toContain('Cannot convert undefined or null to object');
      expect(error.message).toContain('Build failed with 1 error');
      expect(error.message).toContain(`Error: ${label}`);
      expect(error.errors).toHaveLength(1);
      if (!isWasiTest) {
        // Native preserves the exact thrown value; WASI reconstructs it as an Error.
        expect(error.errors[0]).toBe(thrown);
      }
    } finally {
      await build.close();
    }
  });

  test('preserves structured plugin error metadata', async () => {
    const cause = Object.assign(new RangeError('nested plugin cause'), {
      nestedMarker: 17,
    });
    const original = Object.assign(new TypeError('plugin metadata failure'), {
      cause,
      code: 'USER_PLUGIN_CODE',
      customMarker: 'retained',
    });
    const build = await rolldown({
      input: 'entry',
      plugins: [
        {
          name: 'error-metadata-probe',
          resolveId(id) {
            if (id === 'entry') return '\0entry';
          },
          load(id) {
            if (id === '\0entry') return 'export default 1';
          },
          transform(_code, id) {
            if (id === '\0entry') throw original;
          },
        },
      ],
    });

    try {
      const failure = await build.generate().catch((error: unknown) => error);
      const [pluginError] = (failure as { errors?: unknown[] }).errors ?? [];
      expect(pluginError).toBe(original);
      expect(pluginError).toMatchObject({
        code: 'PLUGIN_ERROR',
        pluginCode: 'USER_PLUGIN_CODE',
        plugin: 'error-metadata-probe',
        hook: 'transform',
        id: '\0entry',
        customMarker: 'retained',
      });
      expect(original.stack).toContain('plugin metadata failure');
      expect(original.cause).toBe(cause);
      expect(original.cause).toMatchObject({
        name: 'RangeError',
        message: 'nested plugin cause',
        nestedMarker: 17,
      });
    } finally {
      await build.close();
    }
  });
});

// oxlint-disable no-control-regex
function removeAnsiColors(str: string) {
  return str.replace(/\x1b\[[0-9;]*m/g, '');
}
