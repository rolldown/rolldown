import { expect, test, vi } from 'vitest';
import type { BundlerOptionWithStopWorker } from '../../src/utils/create-bundler-option';
import { createWatcher } from '../../src/api/watch/watcher';
import { WatcherEmitter } from '../../src/api/watch/watch-emitter';

const { createBundlerOptions, createBindingWatcher } = vi.hoisted(() => ({
  createBundlerOptions: vi.fn(),
  createBindingWatcher: vi.fn(),
}));

vi.mock('../../src/utils/create-bundler-option', () => ({ createBundlerOptions }));
vi.mock('../../src/plugin/plugin-driver', () => ({
  PluginDriver: { callOptionsHook: async (options: unknown) => options },
}));
vi.mock('../../src/binding.cjs', () => ({
  BindingWatcher: createBindingWatcher,
  startAsyncRuntime: vi.fn(),
  shutdownAsyncRuntime: vi.fn(),
}));

function setupOptions(stopWorkers: () => Promise<void>): BundlerOptionWithStopWorker {
  return { bundlerOptions: {}, stopWorkers } as BundlerOptionWithStopWorker;
}

test('failed watch setup stops workers from another setup that finishes later', async () => {
  const failure = new Error('options failure');
  const stopWorkers = vi.fn(async () => {});
  let finish!: (options: BundlerOptionWithStopWorker) => void;
  const pending = new Promise<BundlerOptionWithStopWorker>((resolve) => (finish = resolve));
  createBundlerOptions.mockReset();
  createBundlerOptions.mockRejectedValueOnce(failure).mockReturnValueOnce(pending);
  const setup = createWatcher(new WatcherEmitter(), [{}, {}]);
  const rejected = expect(setup).rejects.toBe(failure);
  finish(setupOptions(stopWorkers));
  await rejected;
  expect(stopWorkers).toHaveBeenCalledExactlyOnceWith();
});

test('native watch setup failure stops every worker even when one cleanup rejects', async () => {
  const failure = new Error('native constructor failure');
  const firstStop = vi.fn(async () => {
    throw new Error('worker cleanup failure');
  });
  const secondStop = vi.fn(async () => {});
  createBundlerOptions.mockReset();
  createBundlerOptions
    .mockResolvedValueOnce(setupOptions(firstStop))
    .mockResolvedValueOnce(setupOptions(secondStop));
  createBindingWatcher.mockImplementationOnce(function () {
    throw failure;
  });
  await expect(createWatcher(new WatcherEmitter(), [{}, {}])).rejects.toBe(failure);
  expect(firstStop).toHaveBeenCalledExactlyOnceWith();
  expect(secondStop).toHaveBeenCalledExactlyOnceWith();
});
