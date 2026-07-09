import type { Plugin } from 'rolldown';
import { build, rolldown } from 'rolldown';
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

  test('failed scan closes exactly once with the original diagnostic context', async () => {
    const loadError = new TypeError('retained load failure');
    let receivedError: Error | undefined;
    const closeBundleFn = vi.fn((error?: Error) => {
      receivedError = error;
    });
    const bundle = await rolldown({
      input: './main.js',
      cwd: import.meta.dirname,
      plugins: [
        {
          name: 'failed-scan-close-context',
          load() {
            throw loadError;
          },
          closeBundle: closeBundleFn,
        },
      ],
    });

    const buildError = await bundle.generate().catch((error: unknown) => error);

    expect((buildError as { errors: unknown[] }).errors).toEqual([loadError]);
    expect((receivedError as unknown as { errors: unknown[] }).errors).toEqual([loadError]);
    expect(closeBundleFn).toHaveBeenCalledOnce();
    await expect(bundle.close()).resolves.toBeUndefined();
    expect(closeBundleFn).toHaveBeenCalledOnce();
  });

  test('buildEnd-only failure closes once with the buildEnd diagnostic', async () => {
    const buildEndError = new RangeError('retained buildEnd failure');
    let receivedError: Error | undefined;
    const closeBundleFn = vi.fn((error?: Error) => {
      receivedError = error;
    });
    const bundle = await rolldown({
      input: './main.js',
      cwd: import.meta.dirname,
      plugins: [
        {
          name: 'build-end-only-close-context',
          buildEnd() {
            throw buildEndError;
          },
          closeBundle: closeBundleFn,
        },
      ],
    });

    const buildError = await bundle.generate().catch((error: unknown) => error);

    expect((buildError as { errors: unknown[] }).errors).toEqual([buildEndError]);
    expect((receivedError as unknown as { errors: unknown[] }).errors).toEqual([buildEndError]);
    expect(closeBundleFn).toHaveBeenCalledOnce();
    await expect(bundle.close()).resolves.toBeUndefined();
    expect(closeBundleFn).toHaveBeenCalledOnce();
  });

  test('build and buildEnd failures are both delivered to closeBundle once', async () => {
    const loadError = new TypeError('retained compound load failure');
    const buildEndError = new RangeError('retained compound buildEnd failure');
    let receivedBuildEndError: Error | undefined;
    let receivedCloseError: Error | undefined;
    const closeBundleFn = vi.fn((error?: Error) => {
      receivedCloseError = error;
    });
    const bundle = await rolldown({
      input: './main.js',
      cwd: import.meta.dirname,
      plugins: [
        {
          name: 'compound-build-close-context',
          load() {
            throw loadError;
          },
          buildEnd(error) {
            receivedBuildEndError = error;
            throw buildEndError;
          },
          closeBundle: closeBundleFn,
        },
      ],
    });

    const buildError = await bundle.generate().catch((error: unknown) => error);

    expect((receivedBuildEndError as unknown as { errors: unknown[] }).errors).toEqual([loadError]);
    expect((buildError as { errors: unknown[] }).errors).toEqual([loadError, buildEndError]);
    expect((receivedCloseError as unknown as { errors: unknown[] }).errors).toEqual([
      loadError,
      buildEndError,
    ]);
    expect(closeBundleFn).toHaveBeenCalledOnce();
    await expect(bundle.close()).resolves.toBeUndefined();
    expect(closeBundleFn).toHaveBeenCalledOnce();
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

  test('concurrent and late close replay the original closeBundle error', async () => {
    const entered = deferred();
    const release = deferred();
    const closeError = Object.assign(new RangeError('retained closeBundle failure'), {
      marker: 'original-close-error',
    });
    const closeBundleFn = vi.fn(async () => {
      entered.resolve();
      await release.promise;
      throw closeError;
    });
    const build = await rolldown({
      input: './main.js',
      cwd: import.meta.dirname,
      plugins: [{ name: 'retained-close-error', closeBundle: closeBundleFn }],
    });
    await build.generate();

    const first = build.close();
    await entered.promise;
    const concurrent = build.close();
    expect(concurrent).toBe(first);
    release.resolve();

    await expect(first).rejects.toBe(closeError);
    await expect(concurrent).rejects.toBe(closeError);
    const late = build.close();
    expect(late).toBe(first);
    await expect(late).rejects.toBe(closeError);
    expect(closeBundleFn).toHaveBeenCalledOnce();
  });

  test.each<[string, () => unknown]>([
    ['string', () => 'close string failure'],
    ['number', () => 404],
    ['null', () => null],
    ['plain object', () => ({ marker: 'close object failure' })],
  ])('preserves a %s closeBundle throw through close and build aggregation', async (_, value) => {
    const closeValue = value();
    const closeBundleFn = vi.fn(() => {
      throw closeValue;
    });
    const bundle = await rolldown({
      input: './main.js',
      cwd: import.meta.dirname,
      plugins: [{ name: 'non-error-close-identity', closeBundle: closeBundleFn }],
    });
    await bundle.generate();

    const firstClose = bundle.close();
    await expect(firstClose).rejects.toBe(closeValue);
    expect(bundle.close()).toBe(firstClose);
    await expect(bundle.close()).rejects.toBe(closeValue);
    expect(closeBundleFn).toHaveBeenCalledOnce();

    const buildError = new Error('primary build failure');
    const aggregatedCloseBundleFn = vi.fn(() => {
      throw closeValue;
    });
    const aggregate = await build({
      input: './main.js',
      cwd: import.meta.dirname,
      write: false,
      plugins: [
        {
          name: 'non-error-close-aggregation',
          load() {
            throw buildError;
          },
          closeBundle: aggregatedCloseBundleFn,
        },
      ],
    }).catch((error: unknown) => error);

    expect(aggregate).toBeInstanceOf(AggregateError);
    expect((aggregate as AggregateError).errors[1]).toBe(closeValue);
    expect(aggregatedCloseBundleFn).toHaveBeenCalledOnce();
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
        "[ALREADY_CLOSED] Cannot call bundle.generate() or bundle.write() after bundle.close() has started.
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
test('should print original error if it can not be assigned', async () => {
  const error = await buildWithPlugin({
    name: 'test',
    transform() {
      const proxy = new Proxy({ a: 1 }, {});
      structuredClone(proxy);
    },
  });
  // The exact `DataCloneError: #<Object> could not be cloned` text is
  // V8-specific — other engines (e.g. the WASI/browser lanes) word the
  // structured-clone failure differently, so match loosely.
  expect(error!.message).toMatch(/could not be cloned|DataCloneError/);
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
});

// oxlint-disable no-control-regex
function removeAnsiColors(str: string) {
  return str.replace(/\x1b\[[0-9;]*m/g, '');
}
