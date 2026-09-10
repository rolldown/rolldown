import { beforeEach, expect, test, vi } from 'vitest';

const mocks = vi.hoisted(() => ({
  close: vi.fn(),
  closed: false,
}));

vi.mock('../src/binding.cjs', () => ({
  BindingBundler: class {
    close = mocks.close;
    closeTerminal = mocks.close;

    get closed() {
      return mocks.closed;
    }
  },
  getRuntimeCapabilities: () => ({
    asyncRuntimeBuild: false,
    backend: 'tokio',
    blockOnJsThreadSafe: false,
    devSupported: true,
    flavor: 'MultiThread',
    target: 'native',
    threads: true,
    timers: true,
    wasi: false,
    watchSupported: true,
  }),
}));

vi.mock('../src/utils/create-bundler-option', () => ({
  createBundlerOptions: vi.fn(),
}));

// @ts-ignore This focused unit test intentionally reaches package source outside the test rootDir.
import {
  hasRetryableBuildCleanup,
  retryRolldownBuildCleanup,
  RolldownBuild,
} from '../src/api/rolldown/rolldown-build';

beforeEach(() => {
  mocks.close.mockReset();
  mocks.closed = false;
});

test('bundle close retries native transport rejection before releasing its runtime lease', async () => {
  const closeError = new Error('native close transport rejected');
  const release = vi.fn();
  const build = new RolldownBuild({ input: 'entry.js' }, { release });
  mocks.close.mockRejectedValueOnce(closeError).mockResolvedValue(undefined);

  await expect(build.close()).rejects.toBe(closeError);

  expect(mocks.close).toHaveBeenCalledOnce();
  expect(release).not.toHaveBeenCalled();
  expect(hasRetryableBuildCleanup(build)).toBe(true);

  await expect(build.close()).resolves.toBeUndefined();

  expect(mocks.close).toHaveBeenCalledTimes(2);
  expect(release).toHaveBeenCalledOnce();
  expect(hasRetryableBuildCleanup(build)).toBe(false);
});

test('bundle close replays terminal native diagnostics after releasing ownership', async () => {
  const closeError = new Error('closeBundle failed');
  const release = vi.fn();
  const build = new RolldownBuild({ input: 'entry.js' }, { release });
  mocks.close.mockResolvedValue({
    errors: [{ field0: closeError, type: 'JsError' }],
    isBindingErrors: true,
  });

  await expect(build.close()).rejects.toBe(closeError);
  await expect(build.close()).rejects.toBe(closeError);

  expect(mocks.close).toHaveBeenCalledOnce();
  expect(release).toHaveBeenCalledOnce();
  expect(hasRetryableBuildCleanup(build)).toBe(false);
});

test('bundle cleanup retry excludes terminal diagnostics while preserving public replay', async () => {
  const terminalError = new Error('closeBundle failed');
  const releaseError = new Error('runtime release failed');
  const release = vi
    .fn()
    .mockImplementationOnce(() => {
      throw releaseError;
    })
    .mockImplementation(() => {});
  const build = new RolldownBuild({ input: 'entry.js' }, { release });
  mocks.close.mockResolvedValue({
    errors: [{ field0: terminalError, type: 'JsError' }],
    isBindingErrors: true,
  });

  await expect(build.close()).rejects.toMatchObject({
    errors: [terminalError, releaseError],
  });
  await expect(retryRolldownBuildCleanup(build)).resolves.toEqual([terminalError]);

  await expect(build.close()).rejects.toBe(terminalError);
  expect(mocks.close).toHaveBeenCalledOnce();
  expect(release).toHaveBeenCalledTimes(2);
  expect(hasRetryableBuildCleanup(build)).toBe(false);
});
