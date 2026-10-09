import { isWasiTest } from '@tests/runtime-flavor';
import type { Plugin } from 'rolldown';
import { rolldown } from 'rolldown';
import { describe, expect, test, vi } from 'vitest';

function deferred() {
  let resolve!: () => void;
  const promise = new Promise<void>((resolvePromise) => {
    resolve = resolvePromise;
  });
  return { promise, resolve };
}

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

function delay(ms: number) {
  return new Promise<void>((resolve) => setTimeout(resolve, ms));
}

test('awaits async renderStart hook completion', async () => {
  const entered = deferred();
  const release = deferred();
  const calls: string[] = [];
  const build = await rolldown({
    input: './main.js',
    cwd: import.meta.dirname,
    plugins: [
      {
        name: 'async-render-start',
        async renderStart() {
          calls.push('renderStart:start');
          entered.resolve();
          await release.promise;
          calls.push('renderStart:end');
        },
        generateBundle() {
          calls.push('generateBundle');
        },
      },
    ],
  });

  try {
    let settled = false;
    const generation = build.generate().then(
      (output) => ({ output }),
      (error: unknown) => ({ error }),
    );
    void generation.finally(() => {
      settled = true;
    });

    await entered.promise;
    await delay(20);
    const settledBeforeRelease = settled;
    release.resolve();

    const result = await generation;
    expect(settledBeforeRelease).toBe(false);
    expect(result).toHaveProperty('output');
    expect(calls).toEqual(['renderStart:start', 'renderStart:end', 'generateBundle']);
  } finally {
    release.resolve();
    await build.close();
  }
});

test('propagates async renderStart hook rejection', async () => {
  const build = await rolldown({
    input: './main.js',
    cwd: import.meta.dirname,
    plugins: [
      {
        name: 'async-render-start-rejection',
        async renderStart() {
          await Promise.resolve();
          throw new Error('async renderStart rejection');
        },
      },
    ],
  });

  try {
    await expect(build.generate()).rejects.toThrow('async renderStart rejection');
  } finally {
    await build.close();
  }
});

test('Plugin renderError hook', async () => {
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

test('awaits async renderError hook completion', async () => {
  const entered = deferred();
  const release = deferred();
  let completed = false;
  const build = await rolldown({
    input: './main.js',
    cwd: import.meta.dirname,
    plugins: [
      {
        name: 'async-render-error',
        renderStart() {
          throw new Error('renderStart failure');
        },
        async renderError(error) {
          expect(error.message).toContain('renderStart failure');
          entered.resolve();
          await release.promise;
          completed = true;
        },
      },
    ],
  });

  try {
    let settled = false;
    const generation = build.generate().then(
      (output) => ({ output }),
      (error: unknown) => ({ error }),
    );
    void generation.finally(() => {
      settled = true;
    });

    await entered.promise;
    await delay(20);
    const settledBeforeRelease = settled;
    release.resolve();

    const result = await generation;
    expect(settledBeforeRelease).toBe(false);
    expect(completed).toBe(true);
    expect(result).toHaveProperty('error');
    expect((result as { error: Error }).error.message).toContain('renderStart failure');
  } finally {
    release.resolve();
    await build.close();
  }
});

test('propagates async renderError hook rejection', async () => {
  const build = await rolldown({
    input: './main.js',
    cwd: import.meta.dirname,
    plugins: [
      {
        name: 'async-render-error-rejection',
        renderStart() {
          throw new Error('renderStart failure');
        },
        async renderError() {
          await Promise.resolve();
          throw new Error('async renderError rejection');
        },
      },
    ],
  });

  try {
    await expect(build.generate()).rejects.toThrow('async renderError rejection');
  } finally {
    await build.close();
  }
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
  // `undefined`/`null` reaches the summary formatter verbatim and must not make it
  // throw. See internal-docs/async-runtime/implementation.md, "Plugin error metadata".
  test.each([
    ['undefined', undefined],
    ['null', null],
  ])('summarizes a %s thrown by an output-option callback', async (label, thrown) => {
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
      // the exact thrown value still reaches the caller
      expect(error.errors).toHaveLength(1);
      expect(error.errors[0]).toBe(thrown);
    } finally {
      await build.close();
    }
  });
});

// oxlint-disable no-control-regex
function removeAnsiColors(str: string) {
  return str.replace(/\x1b\[[0-9;]*m/g, '');
}
