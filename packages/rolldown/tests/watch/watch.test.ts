import crypto from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';
import type { RolldownWatcher, RolldownWatcherEvent, WatchOptions } from 'rolldown';
import { rolldown, watch as _watch } from 'rolldown';
import { defineParallelPlugin } from 'rolldown/experimental';
import { sleep } from 'rolldown-tests/utils';
import { isSingleThread } from '@tests/runtime-flavor';
import { test, vi } from 'vitest';

const TEST_RETRY = 3;
const TEST_TIMEOUT = 60_000;

// Wrap watch() to inject usePolling for CI stability.
// PollWatcher uses whole-second mtime comparison, so file edits
// must use editFile() to ensure mtime crosses a second boundary.
function watch(input: WatchOptions | WatchOptions[]) {
  const options = Array.isArray(input) ? input : [input];
  for (const opt of options) {
    const existing = opt.watch && typeof opt.watch === 'object' ? opt.watch : {};
    opt.watch = {
      ...existing,
      watcher: { usePolling: true, pollInterval: 50, ...existing.watcher },
    };
  }
  return _watch(Array.isArray(input) ? options : options[0]);
}

// Write a file with a 1s sleep beforehand to ensure the PollWatcher's
// whole-second mtime comparison detects the change.
async function editFile(filePath: string, content: string) {
  await sleep(1000);
  fs.writeFileSync(filePath, content);
}

// Delete a file with a 1s sleep beforehand (same mtime-boundary reason).
async function deleteFile(filePath: string) {
  await sleep(1000);
  fs.unlinkSync(filePath);
}

test.concurrent(
  'watch',
  { retry: TEST_RETRY, timeout: TEST_TIMEOUT },
  async ({ task, expect, onTestFinished }) => {
    const retryCount = task.result?.retryCount ?? 0;
    const { input, output, dir } = createTestInputAndOutput('watch', retryCount);
    onTestFinished(() => {
      if (!process.env.CI) {
        fs.rmSync(dir, { recursive: true, force: true });
      }
    });
    const foo = path.join(dir, 'foo.js');
    fs.writeFileSync(foo, 'export const foo = 1');
    fs.writeFileSync(input, `import './foo.js'; console.log(1)`);

    const watchChangeUpdateFn = vi.fn();
    const watchChangeCreateFn = vi.fn();
    const watchChangeDeleteFn = vi.fn();
    const closeWatcherFn = vi.fn();
    const watcher = watch({
      input,
      output: { file: output },
      plugins: [
        {
          name: 'test watchChange',
          watchChange(id, event) {
            // The macos emit create event when the file is changed, not sure the reason,
            // so here only check the update event
            if (event.event === 'update') {
              watchChangeUpdateFn();
              expect(id).toBe(input);
            }
            if (event.event === 'create') {
              watchChangeCreateFn();
              expect(id).toBe(foo);
            }
            if (event.event === 'delete') {
              watchChangeDeleteFn();
              expect(id).toBe(foo);
            }
          },
        },
        {
          name: 'test closeWatcher',
          closeWatcher() {
            closeWatcherFn();
          },
        },
      ],
    });

    let errored = false;
    try {
      // should run build once
      await waitBuildFinished(watcher);

      // Test update event
      await editFile(input, `import './foo.js'; console.log(2)`);
      await expect.poll(() => fs.readFileSync(output, 'utf-8')).toContain('console.log(2)');
      // The different platform maybe emit multiple events
      expect(watchChangeUpdateFn).toBeCalled();

      // Test delete event
      await deleteFile(foo);
      await expect.poll(() => watchChangeDeleteFn).toBeCalled();

      // Test create event
      await editFile(foo, 'export const foo = 2');
      await expect.poll(() => watchChangeCreateFn).toBeCalled();
    } catch (e) {
      errored = true;
      throw e;
    } finally {
      await watcher.close();
      if (!errored) {
        expect(closeWatcherFn).toBeCalledTimes(1);
      }
    }
  },
);

test.concurrent(
  'watchChange rejection terminates the watcher and is replayed by close',
  { retry: TEST_RETRY, timeout: TEST_TIMEOUT },
  async ({ task, expect, onTestFinished }) => {
    const retryCount = task.result?.retryCount ?? 0;
    const { input, output, dir } = createTestInputAndOutput('watch-change-rejection', retryCount);
    const hookError = new TypeError('watchChange rejected');
    let markHookCalled!: () => void;
    const hookCalled = new Promise<void>((resolve) => {
      markHookCalled = resolve;
    });
    const watcher = watch({
      input,
      output: { file: output },
      plugins: [
        {
          name: 'watch-change-rejection',
          watchChange() {
            markHookCalled();
            throw hookError;
          },
        },
      ],
    });
    onTestFinished(async () => {
      await watcher.close().catch(() => {});
      if (!process.env.CI) {
        fs.rmSync(dir, { recursive: true, force: true });
      }
    });

    await waitBuildFinished(watcher);
    const closed = new Promise<void>((resolve) => {
      watcher.on('close', resolve);
    });
    await editFile(input, 'console.log(2)');
    await hookCalled;
    await closed;

    await expect(watcher.close()).rejects.toBe(hookError);
    await expect(watcher.close()).rejects.toBe(hookError);
  },
);

test.concurrent(
  'watch files after scan stage',
  { retry: TEST_RETRY, timeout: TEST_TIMEOUT },
  async ({ task, expect, onTestFinished }) => {
    const retryCount = task.result?.retryCount ?? 0;
    const { input, output, dir } = createTestInputAndOutput('watch-files-after-scan', retryCount);
    // Ensure file mtime is in a previous second so PollWatcher detects the renderStart write
    await sleep(1000);
    const watcher = watch({
      input,
      output: { file: output },
      plugins: [
        {
          name: 'test',
          renderStart() {
            fs.writeFileSync(input, 'console.log(2)');
          },
        },
      ],
    });
    onTestFinished(async () => {
      await watcher.close();
      if (!process.env.CI) {
        fs.rmSync(dir, { recursive: true, force: true });
      }
    });
    // should run build once
    await waitBuildFinished(watcher);

    await expect.poll(() => fs.readFileSync(output, 'utf-8')).toContain('console.log(2)');
  },
);

test.concurrent(
  'watch close',
  { retry: TEST_RETRY, timeout: TEST_TIMEOUT },
  async ({ task, expect, onTestFinished }) => {
    const retryCount = task.result?.retryCount ?? 0;
    const { input, output, dir } = createTestInputAndOutput('watch-close', retryCount);
    onTestFinished(() => {
      if (!process.env.CI) {
        fs.rmSync(dir, { recursive: true, force: true });
      }
    });
    const watcher = watch({
      input,
      output: { file: output },
    });
    await waitBuildFinished(watcher);

    await watcher.close();
    // edit file
    fs.writeFileSync(input, 'console.log(3)');
    // The watcher is closed, so the output file should not be updated
    await expect.poll(() => fs.readFileSync(output, 'utf-8')).toContain('console.log(1)');
  },
);

test.concurrent(
  'watcher close in the creation tick runs native cleanup once',
  { retry: TEST_RETRY, timeout: TEST_TIMEOUT },
  async ({ task, expect, onTestFinished }) => {
    const retryCount = task.result?.retryCount ?? 0;
    const { input, output, dir } = createTestInputAndOutput('watch-immediate-close', retryCount);
    onTestFinished(() => {
      if (!process.env.CI) {
        fs.rmSync(dir, { recursive: true, force: true });
      }
    });

    const closeWatcherFn = vi.fn();
    const closeBundleFn = vi.fn();
    const closeEventFn = vi.fn();
    const watcher = watch({
      input,
      output: { file: output },
      plugins: [
        {
          name: 'immediate-close-lifecycle',
          closeWatcher: closeWatcherFn,
          closeBundle: closeBundleFn,
        },
      ],
    });
    watcher.on('close', async () => {
      await Promise.resolve();
      closeEventFn();
    });

    await Promise.all([watcher.close(), watcher.close()]);

    expect(closeWatcherFn).toHaveBeenCalledTimes(1);
    expect(closeBundleFn).not.toHaveBeenCalled();
    expect(closeEventFn).toHaveBeenCalledTimes(1);
    expect(fs.existsSync(output)).toBe(false);
  },
);

test.concurrent(
  'watcher.close() can be awaited from an asynchronous options hook',
  { retry: TEST_RETRY, timeout: TEST_TIMEOUT },
  async ({ task, expect, onTestFinished }) => {
    const retryCount = task.result?.retryCount ?? 0;
    const { input, output, dir } = createTestInputAndOutput(
      'watch-close-inside-options',
      retryCount,
    );
    onTestFinished(() => {
      if (!process.env.CI) {
        fs.rmSync(dir, { recursive: true, force: true });
      }
    });

    let watcher!: RolldownWatcher;
    let optionsHookCompleted = false;
    watcher = watch({
      input,
      output: { file: output },
      plugins: [
        {
          name: 'reentrant-options-close',
          async options(options) {
            await watcher.close();
            optionsHookCompleted = true;
            return options;
          },
        },
      ],
    });

    await watcher.close();

    expect(optionsHookCompleted).toBe(true);
    expect(fs.existsSync(output)).toBe(false);
  },
);

test.concurrent(
  'watcher.close() can be awaited from closeWatcher and closeBundle hooks',
  { retry: TEST_RETRY, timeout: TEST_TIMEOUT },
  async ({ task, expect, onTestFinished }) => {
    const retryCount = task.result?.retryCount ?? 0;
    const { input, output, dir } = createTestInputAndOutput(
      'watch-close-inside-close-hooks',
      retryCount,
    );
    onTestFinished(() => {
      if (!process.env.CI) {
        fs.rmSync(dir, { recursive: true, force: true });
      }
    });

    const completedHooks: string[] = [];
    let watcher!: RolldownWatcher;
    watcher = watch({
      input,
      output: { file: output },
      plugins: [
        {
          name: 'reentrant-watch-close-hooks',
          async closeWatcher() {
            await watcher.close();
            completedHooks.push('closeWatcher');
          },
          async closeBundle() {
            await watcher.close();
            completedHooks.push('closeBundle');
          },
        },
      ],
    });

    await waitBuildFinished(watcher);
    await watcher.close();

    expect(completedHooks).toEqual(['closeWatcher', 'closeBundle']);
  },
);

test.concurrent(
  'cross-watcher close hook cycles acknowledge the watcher lifecycle ancestor',
  { retry: TEST_RETRY, timeout: TEST_TIMEOUT },
  async ({ task, expect, onTestFinished }) => {
    const retryCount = task.result?.retryCount ?? 0;
    const fixtureA = createTestInputAndOutput('watch-cross-close-hook-cycle-a', retryCount);
    const fixtureB = createTestInputAndOutput('watch-cross-close-hook-cycle-b', retryCount);
    const completedHooks: string[] = [];
    let watcherA!: RolldownWatcher;
    let watcherB!: RolldownWatcher;

    watcherA = watch({
      input: fixtureA.input,
      output: { file: fixtureA.output },
      plugins: [
        {
          name: 'cross-close-hook-cycle-a',
          async closeBundle() {
            await Promise.resolve();
            await watcherB.close();
            completedHooks.push('A closeBundle');
          },
        },
      ],
    });
    watcherB = watch({
      input: fixtureB.input,
      output: { file: fixtureB.output },
      plugins: [
        {
          name: 'cross-close-hook-cycle-b',
          async closeWatcher() {
            await Promise.resolve();
            await watcherA.close();
            completedHooks.push('B closeWatcher');
          },
          async closeBundle() {
            await Promise.resolve();
            await watcherA.close();
            completedHooks.push('B closeBundle');
          },
        },
      ],
    });
    onTestFinished(async () => {
      await Promise.allSettled([watcherA.close(), watcherB.close()]);
      if (!process.env.CI) {
        fs.rmSync(fixtureA.dir, { recursive: true, force: true });
        fs.rmSync(fixtureB.dir, { recursive: true, force: true });
      }
    });

    await Promise.all([waitBuildFinished(watcherA), waitBuildFinished(watcherB)]);
    await watcherA.close();
    await watcherB.close();

    expect(completedHooks).toEqual(['B closeWatcher', 'B closeBundle', 'A closeBundle']);
  },
);

test.concurrent(
  'watcher setup failure emits an error and remains closable',
  { retry: TEST_RETRY, timeout: TEST_TIMEOUT },
  async ({ expect }) => {
    const setupError = new Error('watcher setup failed');
    const watcher = watch({
      plugins: [
        {
          name: 'setup-failure',
          async options() {
            await Promise.resolve();
            throw setupError;
          },
        },
      ],
    });

    const events: RolldownWatcherEvent[] = [];
    const closeEventFn = vi.fn();
    const endPromise = new Promise<void>((resolve) => {
      watcher.on('event', (event) => {
        events.push(event);
        if (event.code === 'END') resolve();
      });
    });
    watcher.on('close', closeEventFn);

    await Promise.all([endPromise, watcher.close(), watcher.close()]);

    expect(events).toEqual([{ code: 'ERROR', error: setupError, result: null }, { code: 'END' }]);
    expect(closeEventFn).toHaveBeenCalledTimes(1);
  },
);

test.concurrent(
  'setup-failure close listeners can await close without recursion',
  { retry: TEST_RETRY, timeout: TEST_TIMEOUT },
  async ({ expect }) => {
    const watcher = watch({
      plugins: [
        {
          name: 'setup-failure-reentrant-close',
          options() {
            throw new Error('watcher setup failed');
          },
        },
      ],
    });
    let closeListenerCalls = 0;
    watcher.on('close', async () => {
      await watcher.close();
      closeListenerCalls += 1;
    });

    await watcher.close();
    expect(closeListenerCalls).toBe(1);
  },
);

test.concurrent(
  'watcher close listener failure rejects every concurrent close caller',
  { retry: TEST_RETRY, timeout: TEST_TIMEOUT },
  async ({ task, expect, onTestFinished }) => {
    const retryCount = task.result?.retryCount ?? 0;
    const { input, output, dir } = createTestInputAndOutput(
      'watch-close-listener-failure',
      retryCount,
    );
    onTestFinished(() => {
      if (!process.env.CI) {
        fs.rmSync(dir, { recursive: true, force: true });
      }
    });

    const closeWatcherFn = vi.fn();
    const listenerError = new Error('close listener failed');
    const watcher = watch({
      input,
      output: { file: output },
      plugins: [{ name: 'close-listener-failure', closeWatcher: closeWatcherFn }],
    });
    watcher.on('close', async () => {
      await Promise.resolve();
      throw listenerError;
    });
    const followingListener = vi.fn();
    watcher.on('close', followingListener);

    const results = await Promise.allSettled([watcher.close(), watcher.close()]);
    expect(results).toEqual([
      { status: 'rejected', reason: listenerError },
      { status: 'rejected', reason: listenerError },
    ]);
    expect(closeWatcherFn).toHaveBeenCalledTimes(1);
    expect(followingListener).toHaveBeenCalledTimes(1);
    expect((watcher as any).listeners.size).toBe(0);
  },
);

test.concurrent(
  'watch event listener rejection terminates native lifecycle and is replayed by close',
  { retry: TEST_RETRY, timeout: TEST_TIMEOUT },
  async ({ task, expect, onTestFinished }) => {
    const retryCount = task.result?.retryCount ?? 0;
    const { input, output, dir } = createTestInputAndOutput(
      'watch-event-listener-rejection',
      retryCount,
    );
    const listenerError = new RangeError('watch event listener rejected');
    let markListenerCalled!: () => void;
    const listenerCalled = new Promise<void>((resolve) => {
      markListenerCalled = resolve;
    });
    const watcher = watch({ input, output: { file: output } });
    onTestFinished(async () => {
      await watcher.close().catch(() => {});
      if (!process.env.CI) {
        fs.rmSync(dir, { recursive: true, force: true });
      }
    });

    watcher.on('event', (event) => {
      if (event.code === 'BUNDLE_END') {
        markListenerCalled();
        throw listenerError;
      }
    });
    const closed = new Promise<void>((resolve) => {
      watcher.on('close', resolve);
    });

    await listenerCalled;
    await closed;
    await expect(watcher.close()).rejects.toBe(listenerError);
    await expect(watcher.close()).rejects.toBe(listenerError);
  },
);

test.concurrent(
  'external close during an async close listener awaits and receives its failure',
  { retry: TEST_RETRY, timeout: TEST_TIMEOUT },
  async ({ task, expect, onTestFinished }) => {
    const retryCount = task.result?.retryCount ?? 0;
    const { input, output, dir } = createTestInputAndOutput(
      'watch-close-listener-concurrent-caller',
      retryCount,
    );
    onTestFinished(() => {
      if (!process.env.CI) {
        fs.rmSync(dir, { recursive: true, force: true });
      }
    });

    const listenerError = new Error('close listener failed');
    const watcher = watch({ input, output: { file: output } });
    let markListenerStarted!: () => void;
    const listenerStarted = new Promise<void>((resolve) => {
      markListenerStarted = resolve;
    });
    let releaseListener!: () => void;
    const listenerRelease = new Promise<void>((resolve) => {
      releaseListener = resolve;
    });
    watcher.on('close', async () => {
      markListenerStarted();
      await Promise.resolve();
      await watcher.close();
      await listenerRelease;
      throw listenerError;
    });

    const firstClose = watcher.close();
    await listenerStarted;
    let secondSettled = false;
    const secondClose = watcher.close().finally(() => {
      secondSettled = true;
    });
    await Promise.resolve();
    expect(secondSettled).toBe(false);

    releaseListener();
    await expect(firstClose).rejects.toBe(listenerError);
    await expect(secondClose).rejects.toBe(listenerError);
  },
);

test.concurrent(
  'mutually closing watcher listeners acknowledge an active ancestor',
  { retry: TEST_RETRY, timeout: TEST_TIMEOUT },
  async ({ task, expect, onTestFinished }) => {
    const retryCount = task.result?.retryCount ?? 0;
    const fixtureA = createTestInputAndOutput('watch-close-listener-cycle-a', retryCount);
    const fixtureB = createTestInputAndOutput('watch-close-listener-cycle-b', retryCount);
    const watcherA = watch({
      input: fixtureA.input,
      output: { file: fixtureA.output },
    });
    const watcherB = watch({
      input: fixtureB.input,
      output: { file: fixtureB.output },
    });
    onTestFinished(async () => {
      await Promise.allSettled([watcherA.close(), watcherB.close()]);
      if (!process.env.CI) {
        fs.rmSync(fixtureA.dir, { recursive: true, force: true });
        fs.rmSync(fixtureB.dir, { recursive: true, force: true });
      }
    });

    const closeListenerA = vi.fn(async () => {
      await Promise.resolve();
      await watcherB.close();
    });
    const closeListenerB = vi.fn(async () => {
      await Promise.resolve();
      await watcherA.close();
    });
    watcherA.on('close', closeListenerA);
    watcherB.on('close', closeListenerB);

    await Promise.all([waitBuildFinished(watcherA), waitBuildFinished(watcherB)]);
    await watcherA.close();
    await watcherB.close();

    expect(closeListenerA).toHaveBeenCalledTimes(1);
    expect(closeListenerB).toHaveBeenCalledTimes(1);
  },
);

test.concurrent(
  'detached close-listener descendants use the settled close result',
  { retry: TEST_RETRY, timeout: TEST_TIMEOUT },
  async ({ task, expect, onTestFinished }) => {
    const retryCount = task.result?.retryCount ?? 0;
    const { input, output, dir } = createTestInputAndOutput(
      'watch-close-listener-detached-caller',
      retryCount,
    );
    onTestFinished(() => {
      if (!process.env.CI) {
        fs.rmSync(dir, { recursive: true, force: true });
      }
    });

    const listenerError = new Error('close listener failed');
    const watcher = watch({ input, output: { file: output } });
    let releaseDetachedClose!: () => void;
    const detachedCloseRelease = new Promise<void>((resolve) => {
      releaseDetachedClose = resolve;
    });
    let detachedClose: Promise<void> | undefined;
    watcher.on('close', async () => {
      detachedClose = (async () => {
        await detachedCloseRelease;
        await watcher.close();
      })();
      throw listenerError;
    });

    await expect(watcher.close()).rejects.toBe(listenerError);
    releaseDetachedClose();
    await expect(detachedClose).rejects.toBe(listenerError);
  },
);

test.concurrent(
  'bundle result close waits for one terminal hook result and replays failures',
  { retry: TEST_RETRY, timeout: TEST_TIMEOUT },
  async ({ task, expect, onTestFinished }) => {
    const retryCount = task.result?.retryCount ?? 0;
    const { input, output, dir } = createTestInputAndOutput(
      'watch-bundle-close-failure-replay',
      retryCount,
    );
    onTestFinished(() => {
      if (!process.env.CI) {
        fs.rmSync(dir, { recursive: true, force: true });
      }
    });

    let releaseClose!: () => void;
    const closeRelease = new Promise<void>((resolve) => {
      releaseClose = resolve;
    });
    let markCloseStarted!: () => void;
    const closeStarted = new Promise<void>((resolve) => {
      markCloseStarted = resolve;
    });
    const closeBundleFn = vi.fn(async () => {
      markCloseStarted();
      await closeRelease;
      throw new Error('closeBundle terminal failure');
    });
    const watcher = watch({
      input,
      output: { file: output },
      plugins: [{ name: 'close-failure-replay', closeBundle: closeBundleFn }],
    });

    const resultPromise = new Promise<{ close(): Promise<void> }>((resolve, reject) => {
      watcher.on('event', (event) => {
        if (event.code === 'BUNDLE_END') resolve(event.result);
        if (event.code === 'ERROR') reject(event.error);
      });
    });
    const result = await resultPromise;

    const firstClose = result.close();
    await closeStarted;
    const secondClose = result.close();
    const closeResultsPromise = Promise.allSettled([firstClose, secondClose]);
    let secondSettled = false;
    void secondClose.then(
      () => {
        secondSettled = true;
      },
      () => {
        secondSettled = true;
      },
    );
    await new Promise<void>((resolve) => setImmediate(resolve));
    expect(secondSettled).toBe(false);

    releaseClose();
    const closeResults = await closeResultsPromise;
    expect(closeResults).toHaveLength(2);
    for (const closeResult of closeResults) {
      expect(closeResult.status).toBe('rejected');
      if (closeResult.status === 'rejected') {
        expect(closeResult.reason.message).toContain('closeBundle terminal failure');
      }
    }
    expect(closeBundleFn).toHaveBeenCalledTimes(1);

    await expect(result.close()).rejects.toThrow('closeBundle terminal failure');
    await expect(watcher.close()).rejects.toThrow('closeBundle terminal failure');
    expect(closeBundleFn).toHaveBeenCalledTimes(1);
  },
);

test.concurrent(
  'retained watch results keep per-build plugin resources isolated across rebuilds',
  { retry: TEST_RETRY, timeout: TEST_TIMEOUT },
  async ({ task, expect, onTestFinished }) => {
    const retryCount = task.result?.retryCount ?? 0;
    const { input, outputDir, dir } = createTestInputAndOutput(
      'watch-retained-result-resource-isolation',
      retryCount,
    );
    const references: string[] = [];
    const closedFileNames: string[] = [];
    const results: Array<{ close(): Promise<void> }> = [];
    const watcher = watch({
      input,
      output: { dir: outputDir },
      plugins: [
        {
          name: 'retained-result-resource-isolation',
          buildStart() {
            const build = references.length;
            references.push(
              this.emitFile({
                type: 'asset',
                fileName: `build-${build}.txt`,
                source: `build ${build}`,
              }),
            );
          },
          closeBundle() {
            closedFileNames.push(this.getFileName(references[closedFileNames.length]));
          },
        },
      ],
    });
    watcher.on('event', (event) => {
      if (event.code === 'BUNDLE_END') results.push(event.result);
    });
    onTestFinished(async () => {
      await watcher.close();
      if (!process.env.CI) {
        fs.rmSync(dir, { recursive: true, force: true });
      }
    });

    await waitBuildFinished(watcher);
    await editFile(input, 'console.log(2)');
    await waitBuildFinished(watcher);
    expect(results).toHaveLength(2);

    await results[0].close();
    await results[1].close();
    expect(closedFileNames).toEqual(['build-0.txt', 'build-1.txt']);
  },
);

test
  .skipIf(isSingleThread)
  .concurrent(
    'watcher close closes every retained result before parallel workers terminate',
    { retry: TEST_RETRY, timeout: TEST_TIMEOUT },
    async ({ task, expect, onTestFinished }) => {
      const retryCount = task.result?.retryCount ?? 0;
      const { input, output, dir } = createTestInputAndOutput(
        'watch-retained-parallel-result-close',
        retryCount,
      );
      const state = new Int32Array(new SharedArrayBuffer(Int32Array.BYTES_PER_ELEMENT * 2));
      const parallelPlugin = defineParallelPlugin<{ state: Int32Array }>(
        path.join(import.meta.dirname, '../build-api/parallel-close-plugin.mjs'),
      );
      const results: Array<{ close(): Promise<void> }> = [];
      const watcher = watch({
        input,
        output: { file: output },
        plugins: [parallelPlugin({ state })],
      });
      watcher.on('event', (event) => {
        if (event.code === 'BUNDLE_END') results.push(event.result);
      });
      onTestFinished(async () => {
        await watcher.close().catch(() => {});
        if (!process.env.CI) {
          fs.rmSync(dir, { recursive: true, force: true });
        }
      });

      await waitBuildFinished(watcher);
      await editFile(input, 'console.log(2)');
      await waitBuildFinished(watcher);
      expect(results).toHaveLength(2);
      const expectedCloseBundleCalls = Atomics.load(state, 0);
      expect(expectedCloseBundleCalls).toBeGreaterThan(0);

      await watcher.close();

      expect(Atomics.load(state, 1)).toBe(expectedCloseBundleCalls);
      await Promise.all(results.map((result) => result.close()));
    },
  );

test.concurrent(
  'bundle result close can be re-entered from closeBundle without settling external callers',
  { retry: TEST_RETRY, timeout: TEST_TIMEOUT },
  async ({ task, expect, onTestFinished }) => {
    const retryCount = task.result?.retryCount ?? 0;
    const { input, output, dir } = createTestInputAndOutput(
      'watch-bundle-close-reentrant',
      retryCount,
    );
    let result: { close(): Promise<void> } | undefined;
    let markReentrantCloseCompleted!: () => void;
    const reentrantCloseCompleted = new Promise<void>((resolve) => {
      markReentrantCloseCompleted = resolve;
    });
    let releaseCloseBundle!: () => void;
    const closeBundleRelease = new Promise<void>((resolve) => {
      releaseCloseBundle = resolve;
    });
    const closeBundleFn = vi.fn(async () => {
      if (!result) throw new Error('BUNDLE_END result was not captured');
      await result.close();
      markReentrantCloseCompleted();
      await closeBundleRelease;
    });
    const watcher = watch({
      input,
      output: { file: output },
      plugins: [{ name: 'close-reentrant-result', closeBundle: closeBundleFn }],
    });
    onTestFinished(async () => {
      releaseCloseBundle();
      try {
        await watcher.close();
      } finally {
        if (!process.env.CI) {
          fs.rmSync(dir, { recursive: true, force: true });
        }
      }
    });

    result = await new Promise<{ close(): Promise<void> }>((resolve, reject) => {
      watcher.on('event', (event) => {
        if (event.code === 'BUNDLE_END') resolve(event.result);
        if (event.code === 'ERROR') reject(event.error);
      });
    });
    expect('closeIdentity' in result).toBe(false);

    let externalCloseSettled = false;
    const externalClose = result.close();
    void externalClose.then(
      () => {
        externalCloseSettled = true;
      },
      () => {
        externalCloseSettled = true;
      },
    );

    await reentrantCloseCompleted;
    await new Promise<void>((resolve) => setImmediate(resolve));
    expect(externalCloseSettled).toBe(false);

    releaseCloseBundle();
    await externalClose;
    await result.close();
    expect(closeBundleFn).toHaveBeenCalledTimes(1);
  },
);

test.concurrent(
  'bundle result close awaits a different result and preserves its closeBundle error',
  { retry: TEST_RETRY, timeout: TEST_TIMEOUT },
  async ({ task, expect, onTestFinished }) => {
    const retryCount = task.result?.retryCount ?? 0;
    const { input, output, dir } = createTestInputAndOutput(
      'watch-bundle-close-cross-result',
      retryCount,
    );
    const closeError = Object.assign(new RangeError('cross-result closeBundle failure'), {
      identityMarker: 'preserved',
    });
    let resultB: { close(): Promise<void> } | undefined;
    let closeBundleCalls = 0;
    let observedCloseError: unknown;
    let bCloseFromA: Promise<void> | undefined;
    let aFinishedAwaitingB = false;
    let markBCloseBundleStarted!: () => void;
    const bCloseBundleStarted = new Promise<void>((resolve) => {
      markBCloseBundleStarted = resolve;
    });
    let releaseBCloseBundle!: () => void;
    const bCloseBundleRelease = new Promise<void>((resolve) => {
      releaseBCloseBundle = resolve;
    });
    const closeBundleFn = vi.fn(async () => {
      closeBundleCalls += 1;
      if (closeBundleCalls === 1) {
        if (!resultB) throw new Error('second BUNDLE_END result was not captured');
        try {
          bCloseFromA = resultB.close();
          await bCloseFromA;
        } catch (error) {
          observedCloseError = error;
        }
        aFinishedAwaitingB = true;
        return;
      }
      if (closeBundleCalls === 2) {
        markBCloseBundleStarted();
        await bCloseBundleRelease;
        throw closeError;
      }
      throw new Error(`unexpected closeBundle call ${closeBundleCalls}`);
    });
    const watcher = watch({
      input,
      output: { file: output },
      plugins: [{ name: 'cross-result-close', closeBundle: closeBundleFn }],
    });
    onTestFinished(async () => {
      releaseBCloseBundle();
      try {
        await watcher.close();
      } catch {}
      if (!process.env.CI) {
        fs.rmSync(dir, { recursive: true, force: true });
      }
    });

    let bundleEndCount = 0;
    let resolveResultA!: (result: { close(): Promise<void> }) => void;
    let rejectResultA!: (error: unknown) => void;
    const resultAPromise = new Promise<{ close(): Promise<void> }>((resolve, reject) => {
      resolveResultA = resolve;
      rejectResultA = reject;
    });
    let resolveResultB!: (result: { close(): Promise<void> }) => void;
    let rejectResultB!: (error: unknown) => void;
    const resultBPromise = new Promise<{ close(): Promise<void> }>((resolve, reject) => {
      resolveResultB = resolve;
      rejectResultB = reject;
    });
    watcher.on('event', (event) => {
      if (event.code === 'BUNDLE_END') {
        bundleEndCount += 1;
        if (bundleEndCount === 1) resolveResultA(event.result);
        if (bundleEndCount === 2) resolveResultB(event.result);
      } else if (event.code === 'ERROR') {
        rejectResultA(event.error);
        rejectResultB(event.error);
      }
    });

    const resultA = await resultAPromise;
    await editFile(input, 'console.log(2)');
    resultB = await resultBPromise;

    let aCloseSettled = false;
    const aClose = resultA.close();
    void aClose.then(
      () => {
        aCloseSettled = true;
      },
      () => {
        aCloseSettled = true;
      },
    );

    await bCloseBundleStarted;
    const concurrentBClose = resultB.close();
    expect(concurrentBClose).toBe(bCloseFromA);
    let concurrentBCloseSettled = false;
    void concurrentBClose.then(
      () => {
        concurrentBCloseSettled = true;
      },
      () => {
        concurrentBCloseSettled = true;
      },
    );
    await new Promise<void>((resolve) => setImmediate(resolve));
    expect(aFinishedAwaitingB).toBe(false);
    expect(aCloseSettled).toBe(false);
    expect(concurrentBCloseSettled).toBe(false);

    releaseBCloseBundle();
    await aClose;
    expect(aFinishedAwaitingB).toBe(true);
    expect(observedCloseError).toBe(closeError);
    await expect(concurrentBClose).rejects.toBe(closeError);
    const lateBClose = resultB.close();
    expect(lateBClose).toBe(concurrentBClose);
    await expect(lateBClose).rejects.toBe(closeError);
    await expect(watcher.close()).rejects.toBe(closeError);
    expect(closeError.identityMarker).toBe('preserved');
    expect(closeBundleFn).toHaveBeenCalledTimes(2);
  },
);

test.concurrent(
  'nested cross-result closeBundle cycle acknowledges the active ancestor',
  { retry: TEST_RETRY, timeout: TEST_TIMEOUT },
  async ({ task, expect, onTestFinished }) => {
    const retryCount = task.result?.retryCount ?? 0;
    const { input, output, dir } = createTestInputAndOutput(
      'watch-bundle-close-ancestor-cycle',
      retryCount,
    );
    let resultA: { close(): Promise<void> } | undefined;
    let resultB: { close(): Promise<void> } | undefined;
    let closeBundleCalls = 0;
    const completedHooks: string[] = [];
    let markAncestorCloseAcknowledged!: () => void;
    const ancestorCloseAcknowledged = new Promise<void>((resolve) => {
      markAncestorCloseAcknowledged = resolve;
    });
    let releaseNestedCloseBundle!: () => void;
    const nestedCloseBundleRelease = new Promise<void>((resolve) => {
      releaseNestedCloseBundle = resolve;
    });
    const closeBundleFn = vi.fn(async () => {
      closeBundleCalls += 1;
      if (closeBundleCalls === 1) {
        if (!resultB) throw new Error('second BUNDLE_END result was not captured');
        await resultB.close();
        completedHooks.push('A');
        return;
      }
      if (closeBundleCalls === 2) {
        if (!resultA) throw new Error('first BUNDLE_END result was not captured');
        await Promise.resolve();
        await resultA.close();
        markAncestorCloseAcknowledged();
        await nestedCloseBundleRelease;
        completedHooks.push('B');
        return;
      }
      throw new Error(`unexpected closeBundle call ${closeBundleCalls}`);
    });
    const watcher = watch({
      input,
      output: { file: output },
      plugins: [{ name: 'nested-cross-result-close', closeBundle: closeBundleFn }],
    });
    onTestFinished(async () => {
      releaseNestedCloseBundle();
      await Promise.allSettled([resultA?.close(), resultB?.close(), watcher.close()]);
      if (!process.env.CI) {
        fs.rmSync(dir, { recursive: true, force: true });
      }
    });

    let bundleEndCount = 0;
    let resolveResultA!: (result: { close(): Promise<void> }) => void;
    let rejectResultA!: (error: unknown) => void;
    const resultAPromise = new Promise<{ close(): Promise<void> }>((resolve, reject) => {
      resolveResultA = resolve;
      rejectResultA = reject;
    });
    let resolveResultB!: (result: { close(): Promise<void> }) => void;
    let rejectResultB!: (error: unknown) => void;
    const resultBPromise = new Promise<{ close(): Promise<void> }>((resolve, reject) => {
      resolveResultB = resolve;
      rejectResultB = reject;
    });
    watcher.on('event', (event) => {
      if (event.code === 'BUNDLE_END') {
        bundleEndCount += 1;
        if (bundleEndCount === 1) resolveResultA(event.result);
        if (bundleEndCount === 2) resolveResultB(event.result);
      } else if (event.code === 'ERROR') {
        rejectResultA(event.error);
        rejectResultB(event.error);
      }
    });

    resultA = await resultAPromise;
    await editFile(input, 'console.log(2)');
    resultB = await resultBPromise;

    let resultACloseSettled = false;
    const resultAClose = resultA.close();
    void resultAClose.then(
      () => {
        resultACloseSettled = true;
      },
      () => {
        resultACloseSettled = true;
      },
    );

    await ancestorCloseAcknowledged;
    let resultBCloseSettled = false;
    const resultBClose = resultB.close();
    void resultBClose.then(
      () => {
        resultBCloseSettled = true;
      },
      () => {
        resultBCloseSettled = true;
      },
    );
    await new Promise<void>((resolve) => setImmediate(resolve));
    expect(resultACloseSettled).toBe(false);
    expect(resultBCloseSettled).toBe(false);

    releaseNestedCloseBundle();
    await Promise.all([resultAClose, resultBClose]);
    await Promise.all([resultA.close(), resultB.close(), watcher.close()]);

    expect(completedHooks).toEqual(['B', 'A']);
    expect(closeBundleFn).toHaveBeenCalledTimes(2);
  },
);

test.concurrent(
  'watcher close aggregates native lifecycle and close listener failures',
  { retry: TEST_RETRY, timeout: TEST_TIMEOUT },
  async ({ task, expect, onTestFinished }) => {
    const retryCount = task.result?.retryCount ?? 0;
    const { input, output, dir } = createTestInputAndOutput(
      'watch-close-error-aggregation',
      retryCount,
    );
    onTestFinished(() => {
      if (!process.env.CI) {
        fs.rmSync(dir, { recursive: true, force: true });
      }
    });

    const closeWatcherError = new TypeError('native closeWatcher failure');
    const closeBundleError = new RangeError('native closeBundle failure');
    const closeListenerError = new Error('JavaScript close listener failure');
    const closeWatcherFn = vi.fn(() => {
      throw closeWatcherError;
    });
    const closeBundleFn = vi.fn(() => {
      throw closeBundleError;
    });
    const closeListenerFn = vi.fn(() => {
      throw closeListenerError;
    });
    const watcher = watch({
      input,
      output: { file: output },
      plugins: [
        {
          name: 'close-error-aggregation',
          closeWatcher: closeWatcherFn,
          closeBundle: closeBundleFn,
        },
      ],
    });
    watcher.on('close', closeListenerFn);
    await waitBuildFinished(watcher);

    const firstClose = watcher.close();
    const concurrentClose = watcher.close();
    const [firstResult, concurrentResult] = await Promise.allSettled([firstClose, concurrentClose]);
    expect(firstResult.status).toBe('rejected');
    expect(concurrentResult.status).toBe('rejected');
    if (firstResult.status !== 'rejected' || concurrentResult.status !== 'rejected') return;

    expect(firstResult.reason).toBe(concurrentResult.reason);
    expect(firstResult.reason).toBeInstanceOf(AggregateError);
    const aggregate = firstResult.reason as AggregateError;
    expect(aggregate.errors).toEqual([closeWatcherError, closeBundleError, closeListenerError]);
    expect(closeWatcherFn).toHaveBeenCalledTimes(1);
    expect(closeBundleFn).toHaveBeenCalledTimes(1);
    expect(closeListenerFn).toHaveBeenCalledTimes(1);

    const lateResult = await Promise.allSettled([watcher.close()]);
    expect(lateResult[0]).toEqual({ status: 'rejected', reason: firstResult.reason });
    expect(closeWatcherFn).toHaveBeenCalledTimes(1);
    expect(closeBundleFn).toHaveBeenCalledTimes(1);
    expect(closeListenerFn).toHaveBeenCalledTimes(1);
  },
);

test.concurrent(
  'watcher close reports superseded and current result failures once each',
  { retry: TEST_RETRY, timeout: TEST_TIMEOUT },
  async ({ task, expect, onTestFinished }) => {
    const retryCount = task.result?.retryCount ?? 0;
    const { input, output, dir } = createTestInputAndOutput(
      'watch-close-retained-error-aggregation',
      retryCount,
    );
    const closeError = new Error('shared closeBundle failure');
    const closeBundleFn = vi.fn(() => {
      throw closeError;
    });
    const watcher = watch({
      input,
      output: { file: output },
      plugins: [{ name: 'retained-close-error-aggregation', closeBundle: closeBundleFn }],
    });
    onTestFinished(async () => {
      await watcher.close().catch(() => {});
      if (!process.env.CI) {
        fs.rmSync(dir, { recursive: true, force: true });
      }
    });

    await waitBuildFinished(watcher);
    await editFile(input, 'console.log(2)');
    await waitBuildFinished(watcher);

    const closeResult = await Promise.allSettled([watcher.close()]);
    expect(closeResult[0].status).toBe('rejected');
    if (closeResult[0].status !== 'rejected') return;
    expect(closeResult[0].reason).toBeInstanceOf(AggregateError);
    const errors = (closeResult[0].reason as AggregateError).errors;
    expect(errors).toHaveLength(2);
    expect(errors).toEqual([closeError, closeError]);
    expect(closeBundleFn).toHaveBeenCalledTimes(2);
  },
);

test.concurrent(
  'watcher close owns the prior result after a hidden rebuild replaces the native handle',
  { retry: TEST_RETRY, timeout: TEST_TIMEOUT },
  async ({ task, expect, onTestFinished }) => {
    const retryCount = task.result?.retryCount ?? 0;
    const { input, output, dir } = createTestInputAndOutput(
      'watch-close-hidden-rebuild-result',
      retryCount,
    );
    const priorResultError = new RangeError('prior emitted result closeBundle failure');
    let buildCount = 0;
    let closeBundleCalls = 0;
    let releaseSecondBuild!: () => void;
    const secondBuildRelease = new Promise<void>((resolve) => {
      releaseSecondBuild = resolve;
    });
    let markSecondBuildStarted!: () => void;
    const secondBuildStarted = new Promise<void>((resolve) => {
      markSecondBuildStarted = resolve;
    });
    const watcher = watch({
      input,
      output: { file: output },
      plugins: [
        {
          name: 'hidden-rebuild-result-ownership',
          async buildStart() {
            const bundleId = ++buildCount;
            if (bundleId === 2) {
              markSecondBuildStarted();
              await secondBuildRelease;
            }
          },
          closeBundle() {
            closeBundleCalls += 1;
            if (closeBundleCalls === 2) throw priorResultError;
          },
        },
      ],
    });
    onTestFinished(async () => {
      releaseSecondBuild();
      await watcher.close().catch(() => {});
      if (!process.env.CI) {
        fs.rmSync(dir, { recursive: true, force: true });
      }
    });

    await waitBuildFinished(watcher);
    await editFile(input, 'console.log(2)');
    await secondBuildStarted;

    const closePromise = watcher.close();
    releaseSecondBuild();

    await expect(closePromise).rejects.toBe(priorResultError);
    expect(closeBundleCalls).toBe(2);
  },
);

test.concurrent(
  'closing from BUNDLE_START reports the transferred result failure once',
  { retry: TEST_RETRY, timeout: TEST_TIMEOUT },
  async ({ task, expect, onTestFinished }) => {
    const retryCount = task.result?.retryCount ?? 0;
    const { input, output, dir } = createTestInputAndOutput(
      'watch-close-bundle-start-result-handoff',
      retryCount,
    );
    const closeError = new RangeError('BUNDLE_START result handoff failure');
    const closeBundleFn = vi.fn(() => {
      throw closeError;
    });
    const buildStartFn = vi.fn();
    const watcher = watch({
      input,
      output: { file: output },
      plugins: [
        {
          name: 'bundle-start-result-handoff',
          buildStart: buildStartFn,
          closeBundle: closeBundleFn,
        },
      ],
    });
    onTestFinished(async () => {
      await watcher.close().catch(() => {});
      if (!process.env.CI) {
        fs.rmSync(dir, { recursive: true, force: true });
      }
    });

    let bundleStartCount = 0;
    let closePromise: Promise<void> | undefined;
    let markCloseStarted!: () => void;
    const closeStarted = new Promise<void>((resolve) => {
      markCloseStarted = resolve;
    });
    watcher.on('event', (event) => {
      if (event.code !== 'BUNDLE_START') return;
      bundleStartCount += 1;
      if (bundleStartCount === 2) {
        closePromise = watcher.close();
        markCloseStarted();
      }
    });

    await waitBuildFinished(watcher);
    await editFile(input, 'console.log(2)');
    await closeStarted;

    await expect(closePromise).rejects.toBe(closeError);
    expect(buildStartFn).toHaveBeenCalledTimes(1);
    expect(closeBundleFn).toHaveBeenCalledTimes(1);
  },
);

test.concurrent(
  'closing from a nested BUNDLE_START microtask reports each native task failure once',
  { retry: TEST_RETRY, timeout: TEST_TIMEOUT },
  async ({ task, expect, onTestFinished }) => {
    const retryCount = task.result?.retryCount ?? 0;
    const first = createTestInputAndOutput(
      'watch-close-bundle-start-nested-microtask-a',
      retryCount,
    );
    const secondOutput = path.join(first.dir, 'second.js');
    const closeError = new RangeError('nested BUNDLE_START task close failure');
    const closeBundleFn = vi.fn(() => {
      throw closeError;
    });
    const buildStartFn = vi.fn();
    const watcher = watch({
      input: first.input,
      output: [{ file: first.output }, { file: secondOutput }],
      plugins: [
        {
          name: 'bundle-start-nested-microtask-handoff',
          buildStart: buildStartFn,
          closeBundle: closeBundleFn,
        },
      ],
    });
    onTestFinished(async () => {
      await watcher.close().catch(() => {});
      if (!process.env.CI) {
        fs.rmSync(first.dir, { recursive: true, force: true });
      }
    });

    let bundleStartCount = 0;
    let closePromise: Promise<void> | undefined;
    let markCloseStarted!: () => void;
    const closeStarted = new Promise<void>((resolve) => {
      markCloseStarted = resolve;
    });
    watcher.on('event', (event) => {
      if (event.code !== 'BUNDLE_START') return;
      bundleStartCount += 1;
      if (bundleStartCount !== 3) return;
      queueMicrotask(() => {
        queueMicrotask(() => {
          queueMicrotask(() => {
            closePromise = watcher.close();
            markCloseStarted();
          });
        });
      });
    });

    await waitBuildFinished(watcher);
    await editFile(first.input, 'console.log(2)');
    await closeStarted;

    const closeResult = await Promise.allSettled([closePromise!]);
    expect(closeResult[0].status).toBe('rejected');
    if (closeResult[0].status !== 'rejected') return;
    expect(closeResult[0].reason).toBeInstanceOf(AggregateError);
    expect((closeResult[0].reason as AggregateError).errors).toEqual([closeError, closeError]);
    expect(buildStartFn).toHaveBeenCalledTimes(2);
    expect(closeBundleFn).toHaveBeenCalledTimes(2);
  },
);

test.concurrent(
  'watcher close preserves a singleton JavaScript hook error object',
  { retry: TEST_RETRY, timeout: TEST_TIMEOUT },
  async ({ task, expect, onTestFinished }) => {
    const retryCount = task.result?.retryCount ?? 0;
    const { input, output, dir } = createTestInputAndOutput(
      'watch-close-error-identity',
      retryCount,
    );
    onTestFinished(() => {
      if (!process.env.CI) {
        fs.rmSync(dir, { recursive: true, force: true });
      }
    });

    const closeError = Object.assign(new TypeError('watch close identity'), {
      identityMarker: 'preserved',
    });
    const watcher = watch({
      input,
      output: { file: output },
      plugins: [
        {
          name: 'close-error-identity',
          closeWatcher() {
            throw closeError;
          },
        },
      ],
    });
    await waitBuildFinished(watcher);

    await expect(watcher.close()).rejects.toBe(closeError);
    await expect(watcher.close()).rejects.toBe(closeError);
    expect(closeError.identityMarker).toBe('preserved');
  },
);

test.concurrent(
  'watcher close preserves a singleton JavaScript closeBundle error object',
  { retry: TEST_RETRY, timeout: TEST_TIMEOUT },
  async ({ task, expect, onTestFinished }) => {
    const retryCount = task.result?.retryCount ?? 0;
    const { input, output, dir } = createTestInputAndOutput(
      'watch-close-bundle-error-identity',
      retryCount,
    );
    onTestFinished(() => {
      if (!process.env.CI) {
        fs.rmSync(dir, { recursive: true, force: true });
      }
    });

    const closeError = Object.assign(new RangeError('watch closeBundle identity'), {
      identityMarker: 'preserved',
    });
    const watcher = watch({
      input,
      output: { file: output },
      plugins: [
        {
          name: 'close-bundle-error-identity',
          closeBundle() {
            throw closeError;
          },
        },
      ],
    });
    await waitBuildFinished(watcher);

    await expect(watcher.close()).rejects.toBe(closeError);
    await expect(watcher.close()).rejects.toBe(closeError);
    expect(closeError.identityMarker).toBe('preserved');
  },
);

// https://github.com/rolldown/rolldown/issues/9462
test.concurrent(
  'watcher.close() can be awaited inside an event callback',
  { retry: TEST_RETRY, timeout: TEST_TIMEOUT },
  async ({ task, expect, onTestFinished }) => {
    const retryCount = task.result?.retryCount ?? 0;
    const { input, output, dir } = createTestInputAndOutput('watch-close-inside-event', retryCount);
    onTestFinished(() => {
      if (!process.env.CI) {
        fs.rmSync(dir, { recursive: true, force: true });
      }
    });

    const closeWatcherFn = vi.fn();
    const watcher = watch({
      input,
      output: { file: output },
      plugins: [
        {
          name: 'test closeWatcher',
          async closeWatcher() {
            await sleep(10);
            closeWatcherFn();
          },
        },
      ],
    });

    const closeFn = vi.fn();
    watcher.on('close', async () => {
      // Closing again from the close listener must remain re-entrant as well.
      await watcher.close();
      closeFn();
    });

    const events: string[] = [];
    await new Promise<void>((resolve, reject) => {
      watcher.on('event', async (event) => {
        events.push(event.code);
        if (event.code !== 'BUNDLE_END') return;

        try {
          await event.result.close();
          await watcher.close();

          // close() must not resolve after merely queueing the request. All cleanup and the close
          // event are complete before its promise settles.
          expect(closeWatcherFn).toHaveBeenCalledTimes(1);
          expect(closeFn).toHaveBeenCalledTimes(1);
          expect(events).not.toContain('END');
          resolve();
        } catch (error) {
          reject(error);
        }
      });
    });
  },
);

test.concurrent(
  'watch event',
  { retry: TEST_RETRY, timeout: TEST_TIMEOUT },
  async ({ task, expect, onTestFinished }) => {
    const retryCount = task.result?.retryCount ?? 0;
    const { input, outputDir, dir } = createTestInputAndOutput('watch-event', retryCount);
    onTestFinished(() => {
      if (!process.env.CI) {
        fs.rmSync(dir, { recursive: true, force: true });
      }
    });
    const watcher = watch({
      input,
      output: { dir: outputDir },
      watch: {
        buildDelay: 50,
      },
    });

    const closeFn = vi.fn();
    let errored = false;
    try {
      const events: any[] = [];
      watcher.on('event', (event) => {
        if (event.code === 'BUNDLE_END') {
          expect(event.output).toEqual([outputDir]);
          expect(event.duration).toBeTypeOf('number');
          events.push({ code: 'BUNDLE_END' });
        } else {
          events.push(event);
        }
      });
      const restartFn = vi.fn();
      watcher.on('restart', restartFn);
      watcher.on('close', closeFn);
      const changeFn = vi.fn();
      watcher.on('change', (id, event) => {
        // The macos emit create event when the file is changed, not sure the reason,
        // so here only check the update event
        if (event.event === 'update') {
          changeFn();
          expect(id).toBe(input);
        }
      });

      // test first build event
      await expect
        .poll(() => events)
        .toEqual([
          { code: 'START' },
          { code: 'BUNDLE_START' },
          { code: 'BUNDLE_END' },
          { code: 'END' },
        ]);

      // edit file
      events.length = 0;
      await editFile(input, 'console.log(3)');
      // Note: The different platform maybe emit multiple events
      await expect
        .poll(() => events)
        .toEqual([
          { code: 'START' },
          { code: 'BUNDLE_START' },
          { code: 'BUNDLE_END' },
          { code: 'END' },
        ]);
      expect(restartFn).toBeCalled();
      expect(changeFn).toBeCalled();
    } catch (e) {
      errored = true;
      throw e;
    } finally {
      await watcher.close();
      if (!errored) {
        // the listener is called with async
        await expect.poll(() => closeFn).toBeCalled();
      }
    }
  },
);

test.concurrent(
  'watch event off',
  { retry: TEST_RETRY, timeout: TEST_TIMEOUT },
  async ({ task, expect, onTestFinished }) => {
    const retryCount = task.result?.retryCount ?? 0;
    const { input, outputDir, dir } = createTestInputAndOutput('watch-event-off', retryCount);
    const watcher = watch({
      input,
      output: { dir: outputDir },
      watch: {
        buildDelay: 50,
      },
    });
    const eventFn = vi.fn();
    watcher.on('event', eventFn);
    onTestFinished(async () => {
      await watcher.close();
      if (!process.env.CI) {
        fs.rmSync(dir, { recursive: true, force: true });
      }
    });
    await waitBuildFinished(watcher);
    expect(eventFn).toHaveBeenCalled();

    eventFn.mockClear();
    watcher.off('event', eventFn);

    await editFile(input, 'console.log(12)');
    await waitBuildFinished(watcher);
    expect(eventFn).not.toHaveBeenCalled();
  },
);

test.concurrent(
  'watch BUNDLE_END event result.close() + closeBundle',
  { retry: TEST_RETRY, timeout: TEST_TIMEOUT },
  async ({ task, expect, onTestFinished }) => {
    const retryCount = task.result?.retryCount ?? 0;
    const { input, outputDir, dir } = createTestInputAndOutput(
      'watch-event-close-closeBundle',
      retryCount,
    );
    const closeBundleFn = vi.fn();
    const watcher = watch({
      input,
      output: { dir: outputDir },
      plugins: [
        {
          name: 'test',
          closeBundle: closeBundleFn,
        },
      ],
    });
    watcher.on('event', async (event) => {
      if (event.code === 'BUNDLE_END') {
        await event.result.close();
      }
    });
    onTestFinished(async () => {
      await watcher.close();
      if (!process.env.CI) {
        fs.rmSync(dir, { recursive: true, force: true });
      }
    });
    await waitBuildFinished(watcher);

    expect(closeBundleFn).toBeCalledTimes(1);

    // The `result.close` could be call multiply times.
    await editFile(input, 'console.log(3)');
    await waitBuildFinished(watcher);
    expect(closeBundleFn).toBeCalledTimes(2);
  },
);

test.concurrent(
  'watch ERROR event result.close() + closeBundle',
  { retry: TEST_RETRY, timeout: TEST_TIMEOUT },
  async ({ task, expect, onTestFinished }) => {
    const retryCount = task.result?.retryCount ?? 0;
    const { input, outputDir, dir } = createTestInputAndOutput(
      'watch-event-ERROR-close-closeBundle',
      retryCount,
    );
    const closeBundleFn = vi.fn();
    const watcher = watch({
      input,
      output: { dir: outputDir },
      plugins: [
        {
          name: 'test',
          buildStart() {
            throw new Error('test error');
          },
          closeBundle: closeBundleFn,
        },
      ],
    });
    watcher.on('event', async (event) => {
      if (event.code === 'ERROR') {
        await event.result?.close();
      }
    });
    onTestFinished(async () => {
      await watcher.close();
      if (!process.env.CI) {
        fs.rmSync(dir, { recursive: true, force: true });
      }
    });

    // The failed build runs closeBundle before emitting ERROR; result.close()
    // replays that completed close and releases the retained build resources.
    await expect.poll(() => closeBundleFn).toBeCalledTimes(1);
  },
);

test.concurrent(
  'watch BUNDLE_END event output + "file" option',
  { retry: TEST_RETRY, timeout: TEST_TIMEOUT },
  async ({ task, expect, onTestFinished }) => {
    const retryCount = task.result?.retryCount ?? 0;
    const { input, output, dir } = createTestInputAndOutput('watch-event-file-output', retryCount);
    const watcher = watch({
      input,
      output: { file: output },
    });
    onTestFinished(async () => {
      await watcher.close();
      if (!process.env.CI) {
        fs.rmSync(dir, { recursive: true, force: true });
      }
    });

    const eventFn = vi.fn();
    watcher.on('event', (event) => {
      if (event.code === 'BUNDLE_END') {
        eventFn();
        expect(event.output).toEqual([output]);
      }
    });

    // test first build event
    await expect.poll(() => eventFn).toBeCalled();
  },
);

test.concurrent(
  'watch BUNDLE_END resolves relative "file" output with dot segments',
  { retry: TEST_RETRY, timeout: TEST_TIMEOUT },
  async ({ task, expect, onTestFinished }) => {
    const retryCount = task.result?.retryCount ?? 0;
    const { input, dir } = createTestInputAndOutput('watch-event-relative-file-output', retryCount);
    const relativeOutput = './nested/../dist/main.js';
    const expectedOutput = path.resolve(dir, relativeOutput);
    const watcher = watch({
      cwd: dir,
      input,
      output: { file: relativeOutput },
    });
    onTestFinished(async () => {
      await watcher.close();
      if (!process.env.CI) {
        fs.rmSync(dir, { recursive: true, force: true });
      }
    });

    const eventFn = vi.fn();
    watcher.on('event', (event) => {
      if (event.code === 'BUNDLE_END') {
        eventFn();
        expect(event.output).toEqual([expectedOutput]);
      }
    });

    await expect.poll(() => eventFn).toBeCalled();
  },
);

test.concurrent(
  'watch event avoid deadlock #2806',
  { retry: TEST_RETRY, timeout: TEST_TIMEOUT },
  async ({ task, expect, onTestFinished }) => {
    const retryCount = task.result?.retryCount ?? 0;
    const { input, output, dir } = createTestInputAndOutput(
      'watch-event-avoid-dead-lock',
      retryCount,
    );
    const watcher = watch({
      input,
      output: { file: output },
    });
    onTestFinished(async () => {
      await watcher.close();
      if (!process.env.CI) {
        fs.rmSync(dir, { recursive: true, force: true });
      }
    });

    const testFn = vi.fn();
    let listening = false;
    watcher.on('event', (event) => {
      if (event.code === 'BUNDLE_END' && !listening) {
        listening = true;
        // shouldn't deadlock
        watcher.on('event', (innerEvent) => {
          if (innerEvent.code === 'BUNDLE_END') {
            testFn();
          }
        });
      }
    });

    await waitBuildFinished(watcher);

    await editFile(input, 'console.log(2)');
    await expect.poll(() => testFn).toBeCalled();
  },
);

test.concurrent(
  'watch skipWrite',
  { retry: TEST_RETRY, timeout: TEST_TIMEOUT },
  async ({ task, expect, onTestFinished }) => {
    const retryCount = task.result?.retryCount ?? 0;
    const { input, output, dir } = createTestInputAndOutput('watch-skipWrite', retryCount);
    const watcher = watch({
      input,
      output: { file: output },
      watch: {
        skipWrite: true,
      },
    });
    onTestFinished(async () => {
      await watcher.close();
      if (!process.env.CI) {
        fs.rmSync(dir, { recursive: true, force: true });
      }
    });
    await waitBuildFinished(watcher);

    expect(fs.existsSync(output)).toBe(false);
  },
);

test.concurrent(
  '#5260',
  { retry: TEST_RETRY, timeout: TEST_TIMEOUT },
  async ({ task, onTestFinished }) => {
    const retryCount = task.result?.retryCount ?? 0;
    const { dir: cwd } = createTestWithMultiFiles('issue-5260', retryCount, {
      'main.js': `import './foo.js'`,
      'foo.js': `console.log('foo')`,
    });
    onTestFinished(() => {
      if (!process.env.CI) {
        fs.rmSync(cwd, { recursive: true, force: true });
      }
    });
    const watcher = watch({
      cwd,
      input: 'main.js',
      watch: {
        buildDelay: 50,
      },
      experimental: {
        incrementalBuild: true,
      },
    });
    onTestFinished(async () => await watcher.close());
    await waitBuildFinished(watcher);

    watcher.clear('event');

    await editFile(path.join(cwd, 'main.js'), `import('./foo.js')`);

    await waitBuildFinished(watcher);
  },
);

test.concurrent(
  'incremental-watch-modify-entry-module',
  { retry: TEST_RETRY, timeout: TEST_TIMEOUT },
  async ({ task, expect, onTestFinished }) => {
    const retryCount = task.result?.retryCount ?? 0;
    const { dir: cwd } = createTestWithMultiFiles(
      'incremental-watch-modify-entry-module',
      retryCount,
      {
        'main.js': `
import {a} from './foo.js'
console.log(a)
`,
        'foo.js': `export const a = 10000`,
      },
    );
    onTestFinished(() => {
      if (!process.env.CI) {
        fs.rmSync(cwd, { recursive: true, force: true });
      }
    });
    const watcher = watch({
      cwd,
      input: 'main.js',
      watch: {
        buildDelay: 50,
      },
      experimental: {
        incrementalBuild: true,
      },
    });
    onTestFinished(async () => await watcher.close());
    await waitBuildFinished(watcher);

    watcher.clear('event');
    expect(fs.readdirSync(path.join(cwd, 'dist'))).toHaveLength(1);

    await editFile(
      path.join(cwd, 'main.js'),
      `
import {a} from './foo.js'
console.log(a + 1000)
`,
    );

    await waitBuildFinished(watcher);
    expect(fs.readdirSync(path.join(cwd, 'dist'))).toHaveLength(1);
  },
);

test.concurrent(
  'watch sync ast of newly added ast',
  { retry: TEST_RETRY, timeout: TEST_TIMEOUT },
  async ({ task, onTestFinished }) => {
    const retryCount = task.result?.retryCount ?? 0;
    const { dir: cwd } = createTestWithMultiFiles('sync-ast-of-newly-added-modules', retryCount, {
      'main.js': `import ('./d1.js').then(console.log)`,
      'd1.js': `export const a = 1`,
      'd2.js': `export const b = 2`,
    });
    onTestFinished(() => {
      if (!process.env.CI) {
        fs.rmSync(cwd, { recursive: true, force: true });
      }
    });
    const watcher = watch({
      cwd,
      input: 'main.js',
      watch: {
        buildDelay: 50,
      },
      experimental: {
        incrementalBuild: true,
      },
    });
    onTestFinished(async () => await watcher.close());
    await waitBuildFinished(watcher);

    watcher.clear('event');

    await editFile(
      path.join(cwd, 'main.js'),
      `import ('./d1.js').then(console.log);import ('./d2.js').then(console.log)`,
    );

    await waitBuildFinished(watcher);
  },
);

test.concurrent(
  'watch buildDelay',
  { retry: TEST_RETRY, timeout: TEST_TIMEOUT },
  async ({ task, expect, onTestFinished }) => {
    const retryCount = task.result?.retryCount ?? 0;
    const { input, output, dir } = createTestInputAndOutput('watch-buildDelay', retryCount);
    const buildDelay = 500;
    let resolveFirstInvalidation!: () => void;
    const firstInvalidation = new Promise<void>((resolve) => {
      resolveFirstInvalidation = resolve;
    });
    const onInvalidateFn = vi.fn(resolveFirstInvalidation);
    const watcher = watch({
      input,
      output: { file: output },
      watch: {
        buildDelay,
        onInvalidate: onInvalidateFn,
        watcher: {
          pollInterval: 10,
          compareContentsForPolling: true,
        },
      },
    });
    onTestFinished(async () => {
      await watcher.close();
      if (!process.env.CI) {
        fs.rmSync(dir, { recursive: true, force: true });
      }
    });
    await waitBuildFinished(watcher);

    const restartFn = vi.fn();
    watcher.on('restart', restartFn);

    const rebuildFinished = waitBuildFinished(watcher);
    fs.writeFileSync(input, 'console.log(4)');
    await firstInvalidation;
    fs.writeFileSync(input, 'console.log(5)');

    await expect.poll(() => onInvalidateFn).toHaveBeenCalledTimes(2);
    await rebuildFinished;
    await sleep(buildDelay + 50);
    expect(fs.readFileSync(output, 'utf-8')).toContain('console.log(5)');
    expect(restartFn).toBeCalledTimes(1);
  },
);

test.concurrent(
  'PluginContext addWatchFile',
  { retry: TEST_RETRY, timeout: TEST_TIMEOUT },
  async ({ task, expect, onTestFinished }) => {
    const retryCount = task.result?.retryCount ?? 0;
    const { input, output, dir } = createTestInputAndOutput('addWatchFile', retryCount);
    const { input: foo, dir: fooDir } = createTestInputAndOutput('addWatchFile-foo', retryCount);
    onTestFinished(() => {
      if (!process.env.CI) {
        fs.rmSync(dir, { recursive: true, force: true });
        fs.rmSync(fooDir, { recursive: true, force: true });
      }
    });
    const watcher = watch({
      input,
      output: { file: output },
      plugins: [
        {
          name: 'test',
          buildStart() {
            this.addWatchFile(foo);
          },
        },
      ],
    });
    onTestFinished(async () => await watcher.close());

    await waitBuildFinished(watcher);

    const changeFn = vi.fn();
    watcher.on('change', (id, event) => {
      // The macos emit create event when the file is changed, not sure the reason,
      // so here only check the update event
      if (event.event === 'update') {
        changeFn();
        expect(id).toBe(foo);
      }
    });

    // edit file
    await editFile(foo, 'console.log(2)\n');
    await expect.poll(() => changeFn).toBeCalled();
  },
);

test.concurrent(
  'watch include/exclude',
  { retry: TEST_RETRY, timeout: TEST_TIMEOUT },
  async ({ task, expect, onTestFinished }) => {
    const retryCount = task.result?.retryCount ?? 0;
    const { input, output, dir } = createTestInputAndOutput('include-exclude', retryCount);
    const watcher = watch({
      input,
      output: { file: output },
      watch: {
        exclude: 'main.js',
      },
    });
    onTestFinished(async () => {
      await watcher.close();
      if (!process.env.CI) {
        fs.rmSync(dir, { recursive: true, force: true });
      }
    });

    await waitBuildFinished(watcher);

    // edit file
    await editFile(input, 'console.log(2)');
    // The input is excluded, so the output file should not be updated
    await expect.poll(() => fs.readFileSync(output, 'utf-8')).toContain('console.log(1)');
  },
);

test.concurrent(
  'watch onInvalidate',
  { retry: TEST_RETRY, timeout: TEST_TIMEOUT },
  async ({ task, expect, onTestFinished }) => {
    const retryCount = task.result?.retryCount ?? 0;
    const { input, output, dir } = createTestInputAndOutput('on-invalidate', retryCount);

    const onInvalidateFn = vi.fn();
    const watcher = watch({
      input,
      output: { file: output },
      watch: {
        onInvalidate: (id) => {
          expect(id).toBe(input);
          onInvalidateFn(id);
        },
      },
    });
    onTestFinished(async () => {
      await watcher.close();
      if (!process.env.CI) {
        fs.rmSync(dir, { recursive: true, force: true });
      }
    });

    await waitBuildFinished(watcher);

    // edit file
    await editFile(input, 'console.log(2)');

    await expect.poll(() => fs.readFileSync(output, 'utf-8')).toContain('console.log(2)');
    expect(onInvalidateFn).toBeCalled();
  },
);

test.concurrent(
  'error handling',
  { retry: TEST_RETRY, timeout: TEST_TIMEOUT },
  async ({ task, expect, onTestFinished }) => {
    const retryCount = task.result?.retryCount ?? 0;
    // first build error, the watching could be work with recover error
    const { input, output, dir } = createTestInputAndOutput(
      'error-handling',
      retryCount,
      'conso le.log(1)',
    );

    const watcher = watch({
      input,
      output: { file: output },
    });
    onTestFinished(async () => {
      await watcher.close();
      if (!process.env.CI) {
        fs.rmSync(dir, { recursive: true, force: true });
      }
    });
    const errors: string[] = [];
    watcher.on('event', (event) => {
      if (event.code === 'ERROR') {
        errors.push(event.error.message);
      }
    });
    // First build should error
    await expect.poll(() => errors.length).toBe(1);
    expect(errors[0]).toContain('PARSE_ERROR');

    await editFile(input, 'console.log(2)');
    await waitBuildFinished(watcher);

    // failed again
    await editFile(input, 'conso le.log(1)');
    // The different platform maybe emit multiple events
    await expect.poll(() => errors.length).toBeGreaterThan(0);
    expect(errors[0]).toContain('PARSE_ERROR');

    // It should be working if the changes are fixed error
    await editFile(input, 'console.log(3)');
    await expect.poll(() => fs.readFileSync(output, 'utf-8')).toContain('console.log(3)');
  },
);

test.concurrent(
  'error handling + plugin error',
  { retry: TEST_RETRY, timeout: TEST_TIMEOUT },
  async ({ task, expect, onTestFinished }) => {
    const retryCount = task.result?.retryCount ?? 0;
    const { input, output, dir } = createTestInputAndOutput(
      'error-handling-plugin-error',
      retryCount,
    );
    const watcher = watch({
      input,
      output: { file: output },
      plugins: [
        {
          name: 'test',
          transform() {
            this.error('plugin error');
          },
        },
      ],
    });
    onTestFinished(async () => {
      await watcher.close();
      if (!process.env.CI) {
        fs.rmSync(dir, { recursive: true, force: true });
      }
    });
    const errors: string[] = [];
    watcher.on('event', (event) => {
      if (event.code === 'ERROR') {
        errors.push(event.error.message);
      }
    });
    // First build should error
    // the revert change maybe emit the change event caused it failed
    await expect.poll(() => errors.length).toBe(1);
    expect(errors[0]).toContain('plugin error');

    errors.length = 0;
    await editFile(input, 'console.log(2)');
    // The different platform maybe emit multiple events
    await expect.poll(() => errors.length).toBeGreaterThan(0);
    expect(errors[0]).toContain('plugin error');
  },
);

test.concurrent(
  'empty output array falls back to one default output',
  { retry: TEST_RETRY, timeout: TEST_TIMEOUT },
  async ({ task, expect, onTestFinished }) => {
    const retryCount = task.result?.retryCount ?? 0;
    const { input, dir } = createTestInputAndOutput('watch-empty-output-array', retryCount);
    const watcher = watch({
      input,
      output: [],
      watch: { skipWrite: true },
    });
    onTestFinished(async () => {
      await watcher.close().catch(() => {});
      if (!process.env.CI) {
        fs.rmSync(dir, { recursive: true, force: true });
      }
    });

    let bundleEndCount = 0;
    watcher.on('event', (event) => {
      if (event.code === 'BUNDLE_END') bundleEndCount += 1;
    });

    await expect.poll(() => bundleEndCount).toBe(1);
  },
);

test.concurrent(
  'multi-output watch runs config hooks once and context hooks per output',
  { retry: TEST_RETRY, timeout: TEST_TIMEOUT },
  async ({ task, expect, onTestFinished }) => {
    const retryCount = task.result?.retryCount ?? 0;
    const { input, output, dir } = createTestInputAndOutput(
      'watch-multi-output-config-hooks',
      retryCount,
    );
    const secondOutput = path.join(dir, 'dist', 'second.js');
    const optionsFn = vi.fn();
    const outputOptionsFn = vi.fn();
    const buildStartFn = vi.fn();
    const watchChangeFn = vi.fn();
    const closeWatcherFn = vi.fn();
    const onInvalidateFn = vi.fn();
    const watcher = watch({
      input,
      output: [{ file: output }, { file: secondOutput }],
      watch: {
        buildDelay: 100,
        onInvalidate(id) {
          if (id === input) onInvalidateFn();
        },
      },
      plugins: [
        {
          name: 'multi-output-config-hooks',
          options(options) {
            optionsFn();
            return options;
          },
          outputOptions(options) {
            outputOptionsFn();
            return options;
          },
          buildStart() {
            buildStartFn();
          },
          watchChange(id) {
            if (id === input) watchChangeFn();
          },
          closeWatcher() {
            closeWatcherFn();
          },
        },
      ],
    });
    onTestFinished(async () => {
      await watcher.close().catch(() => {});
      if (!process.env.CI) {
        fs.rmSync(dir, { recursive: true, force: true });
      }
    });

    let bundleEndCount = 0;
    watcher.on('event', (event) => {
      if (event.code === 'BUNDLE_END') bundleEndCount += 1;
    });

    await expect.poll(() => bundleEndCount, { timeout: 10_000 }).toBe(2);
    expect(optionsFn).toHaveBeenCalledOnce();
    expect(outputOptionsFn).toHaveBeenCalledTimes(2);
    expect(buildStartFn).toHaveBeenCalledTimes(2);

    await editFile(input, 'console.log(2)');
    await expect.poll(() => bundleEndCount, { timeout: 10_000 }).toBe(4);
    expect(optionsFn).toHaveBeenCalledOnce();
    expect(outputOptionsFn).toHaveBeenCalledTimes(2);
    expect(buildStartFn).toHaveBeenCalledTimes(4);
    expect(watchChangeFn).toHaveBeenCalledTimes(2);
    expect(onInvalidateFn).toHaveBeenCalledOnce();

    await watcher.close();
    expect(closeWatcherFn).toHaveBeenCalledOnce();
  },
);

test.concurrent(
  'watch multiply options',
  { retry: TEST_RETRY, timeout: TEST_TIMEOUT },
  async ({ task, expect, onTestFinished }) => {
    const retryCount = task.result?.retryCount ?? 0;
    const { input, output, outputDir, dir } = createTestInputAndOutput(
      'watch-multiply-options',
      retryCount,
    );
    const {
      input: foo,
      outputDir: fooOutputDir,
      dir: fooDir,
    } = createTestInputAndOutput('watch-multiply-options-foo', retryCount);
    onTestFinished(() => {
      if (!process.env.CI) {
        fs.rmSync(dir, { recursive: true, force: true });
        fs.rmSync(fooDir, { recursive: true, force: true });
      }
    });
    const watcher = watch([
      {
        input,
        output: { dir: outputDir },
      },
      {
        input: foo,
        output: { dir: fooOutputDir },
      },
    ]);
    onTestFinished(async () => await watcher.close());

    const events: string[] = [];
    watcher.on('event', (event) => {
      if (event.code === 'BUNDLE_END') {
        events.push(event.output[0]);
      }
    });

    // here should using waitBuildFinished to wait the build finished, because the `input` could be finished before `foo`
    // await waitBuildFinished(watcher)
    await expect.poll(() => fs.readFileSync(output, 'utf-8')).toContain('console.log(1)');

    await editFile(input, 'console.log(2)');
    await expect.poll(() => fs.readFileSync(output, 'utf-8')).toContain('console.log(2)');
    // Only the input corresponding bundler is rebuild
    expect(events[0]).toEqual(outputDir);
  },
);

test.concurrent(
  'multiple outputs in one config do not trigger the multiple polling warning',
  { retry: TEST_RETRY, timeout: TEST_TIMEOUT },
  async ({ task, expect, onTestFinished }) => {
    const retryCount = task.result?.retryCount ?? 0;
    const { input, output, dir } = createTestInputAndOutput(
      'watch-multi-output-polling-warning',
      retryCount,
    );
    const secondOutput = path.join(dir, 'dist', 'second.js');
    const multipleWatcherWarnings = vi.fn();
    const watcher = watch({
      input,
      output: [{ file: output }, { file: secondOutput }],
      plugins: [
        {
          name: 'multi-output-polling-warning',
          onLog(_level, log) {
            if (log.code === 'MULTIPLE_WATCHER_OPTION') multipleWatcherWarnings();
          },
        },
      ],
    });
    onTestFinished(async () => {
      await watcher.close().catch(() => {});
      if (!process.env.CI) {
        fs.rmSync(dir, { recursive: true, force: true });
      }
    });

    let bundleEndCount = 0;
    watcher.on('event', (event) => {
      if (event.code === 'BUNDLE_END') bundleEndCount += 1;
    });
    await expect.poll(() => bundleEndCount).toBe(2);
    expect(multipleWatcherWarnings).not.toHaveBeenCalled();
  },
);

test.concurrent(
  'warning for multiply notify options',
  { retry: TEST_RETRY, timeout: TEST_TIMEOUT },
  async ({ task, expect, onTestFinished }) => {
    const retryCount = task.result?.retryCount ?? 0;
    const { input, output, dir } = createTestInputAndOutput(
      'watch-multiply-options-warning',
      retryCount,
    );
    const { input: foo, dir: fooDir } = createTestInputAndOutput(
      'watch-multiply-options-warning-foo',
      retryCount,
    );
    onTestFinished(() => {
      if (!process.env.CI) {
        fs.rmSync(dir, { recursive: true, force: true });
        fs.rmSync(fooDir, { recursive: true, force: true });
      }
    });
    const onLogFn = vi.fn();
    const watcher = watch([
      {
        input,
        output: { file: output },
        watch: {
          watcher: {
            usePolling: true,
            pollInterval: 50,
          },
        },
      },
      {
        input: foo,
        output: { file: output },
        watch: {
          watcher: {
            usePolling: true,
            pollInterval: 100,
          },
        },
        plugins: [
          {
            name: 'test',
            onLog: (level, log) => {
              onLogFn();
              expect(level).toBe('warn');
              expect(log.code).toBe('MULTIPLE_WATCHER_OPTION');
            },
          },
        ],
      },
    ]);
    onTestFinished(async () => await watcher.close());

    await expect.poll(() => onLogFn).toBeCalled();
  },
);

if (process.platform === 'win32') {
  test.concurrent(
    'watch linux path at windows #4385',
    { retry: TEST_RETRY, timeout: TEST_TIMEOUT },
    async ({ task, expect, onTestFinished }) => {
      const retryCount = task.result?.retryCount ?? 0;
      const { input, output, dir } = createTestInputAndOutput(
        'watch-linux-path-at-windows',
        retryCount,
      );
      const watcher = watch({
        input,
        output: { file: output },
        plugins: [
          {
            name: 'test',
            resolveId() {
              return input.replace(/\\/g, '/');
            },
          },
        ],
      });
      onTestFinished(async () => {
        await watcher.close();
        if (!process.env.CI) {
          fs.rmSync(dir, { recursive: true, force: true });
        }
      });
      // should run build once
      await waitBuildFinished(watcher);

      // edit file
      await editFile(input, 'console.log(2)');
      await expect.poll(() => fs.readFileSync(output, 'utf-8')).toContain('console.log(2)');
    },
  );
}

test.concurrent(
  'watch close immediately',
  { retry: TEST_RETRY, timeout: TEST_TIMEOUT },
  async ({ task, onTestFinished }) => {
    const retryCount = task.result?.retryCount ?? 0;
    const { input, output, dir } = createTestInputAndOutput('watch-close-immediately', retryCount);
    onTestFinished(() => {
      if (!process.env.CI) {
        fs.rmSync(dir, { recursive: true, force: true });
      }
    });
    const watcher = watch({
      input,
      output: { file: output },
    });

    await watcher.close();
  },
);

test.concurrent(
  'ids loaded via load hook should not be watched',
  { retry: TEST_RETRY, timeout: TEST_TIMEOUT },
  async ({ task, expect, onTestFinished }) => {
    const retryCount = task.result?.retryCount ?? 0;
    const { dir: cwd } = createTestWithMultiFiles('watchFiles-load-hook', retryCount, {
      'main.js': `import './loaded.js'`,
      'loaded.js': `console.log('on disk')`,
    });
    onTestFinished(() => {
      if (!process.env.CI) {
        fs.rmSync(cwd, { recursive: true, force: true });
      }
    });

    const bundle = await rolldown({
      cwd,
      input: 'main.js',
      plugins: [
        {
          name: 'test-load',
          load(id) {
            if (id.endsWith('loaded.js')) {
              return `console.log('from load hook')`;
            }
          },
        },
      ],
    });
    await bundle.generate();
    const watchFiles = await bundle.watchFiles;
    await bundle.close();

    const normalized = watchFiles.map((f) => f.replace(/\\/g, '/'));
    expect(normalized).toContainEqual(expect.stringContaining('main.js'));
    expect(normalized).not.toContainEqual(expect.stringContaining('loaded.js'));
  },
);

test.concurrent(
  'ids loaded by file read should be watched',
  { retry: TEST_RETRY, timeout: TEST_TIMEOUT },
  async ({ task, expect, onTestFinished }) => {
    const retryCount = task.result?.retryCount ?? 0;
    const { dir: cwd } = createTestWithMultiFiles('watchFiles-file-read', retryCount, {
      'main.js': `import './dep.js'`,
      'dep.js': `console.log('dep')`,
    });
    onTestFinished(() => {
      if (!process.env.CI) {
        fs.rmSync(cwd, { recursive: true, force: true });
      }
    });

    const bundle = await rolldown({ cwd, input: 'main.js' });
    await bundle.generate();
    const watchFiles = await bundle.watchFiles;
    await bundle.close();

    const normalized = watchFiles.map((f) => f.replace(/\\/g, '/'));
    expect(normalized).toContainEqual(expect.stringContaining('main.js'));
    expect(normalized).toContainEqual(expect.stringContaining('dep.js'));
  },
);

test.concurrent(
  'ids added via addWatchFile should be watched',
  { retry: TEST_RETRY, timeout: TEST_TIMEOUT },
  async ({ task, expect, onTestFinished }) => {
    const retryCount = task.result?.retryCount ?? 0;
    const { dir: cwd } = createTestWithMultiFiles('watchFiles-addWatchFile', retryCount, {
      'main.js': `console.log('hello')`,
      'external.txt': 'some data',
    });
    onTestFinished(() => {
      if (!process.env.CI) {
        fs.rmSync(cwd, { recursive: true, force: true });
      }
    });
    const externalFile = path.join(cwd, 'external.txt');

    const bundle = await rolldown({
      cwd,
      input: 'main.js',
      plugins: [
        {
          name: 'test-addWatchFile',
          buildStart() {
            this.addWatchFile(externalFile);
          },
        },
      ],
    });
    await bundle.generate();
    const watchFiles = await bundle.watchFiles;
    await bundle.close();

    const normalized = watchFiles.map((f) => f.replace(/\\/g, '/'));
    expect(normalized).toContainEqual(expect.stringContaining('main.js'));
    expect(normalized).toContainEqual(expect.stringContaining('external.txt'));
  },
);

test.concurrent(
  'watch import non-existing file then create it',
  { retry: TEST_RETRY, timeout: TEST_TIMEOUT },
  async ({ task, expect, onTestFinished }) => {
    const retryCount = task.result?.retryCount ?? 0;
    const { dir: cwd } = createTestWithMultiFiles('import-non-existing-then-create', retryCount, {
      'main.js': `console.log('main')`,
    });
    onTestFinished(() => {
      if (!process.env.CI) {
        fs.rmSync(cwd, { recursive: true, force: true });
      }
    });
    const watcher = watch({
      cwd,
      input: 'main.js',
      output: { dir: path.join(cwd, 'dist') },
    });
    onTestFinished(async () => await watcher.close());
    await waitBuildFinished(watcher);

    // Edit main.js to import a non-existing file — should cause an error
    const errors: string[] = [];
    watcher.on('event', (event) => {
      if (event.code === 'ERROR') {
        errors.push(event.error.message);
      }
    });
    await editFile(path.join(cwd, 'main.js'), `import { foo } from './foo.js'\nconsole.log(foo)`);
    await expect.poll(() => errors.length).toBeGreaterThan(0);

    // Create the missing file, then do a noop edit to main.js to trigger rebuild
    // (the missing file's directory is not auto-watched, so we need to touch a watched file)
    await editFile(path.join(cwd, 'foo.js'), `export const foo = 'added'`);
    await editFile(path.join(cwd, 'main.js'), `import { foo } from './foo.js'\nconsole.log(foo)`);
    await waitBuildFinished(watcher);

    const output = path.join(cwd, 'dist', 'main.js');
    expect(fs.readFileSync(output, 'utf-8')).toContain('added');
  },
);

test.concurrent(
  'watch import non-existing file then rename to it',
  { retry: TEST_RETRY, timeout: TEST_TIMEOUT },
  async ({ task, expect, onTestFinished }) => {
    const retryCount = task.result?.retryCount ?? 0;
    const { dir: cwd } = createTestWithMultiFiles('import-non-existing-then-rename', retryCount, {
      'main.js': `console.log('main')`,
      'bar.js': `export const foo = 'renamed'`,
    });
    onTestFinished(() => {
      if (!process.env.CI) {
        fs.rmSync(cwd, { recursive: true, force: true });
      }
    });
    const watcher = watch({
      cwd,
      input: 'main.js',
      output: { dir: path.join(cwd, 'dist') },
    });
    onTestFinished(async () => await watcher.close());
    await waitBuildFinished(watcher);

    // Edit main.js to import a non-existing file — should cause an error
    const errors: string[] = [];
    watcher.on('event', (event) => {
      if (event.code === 'ERROR') {
        errors.push(event.error.message);
      }
    });
    await editFile(path.join(cwd, 'main.js'), `import { foo } from './foo.js'\nconsole.log(foo)`);
    await expect.poll(() => errors.length).toBeGreaterThan(0);

    // Rename bar.js to foo.js, then do a noop edit to main.js to trigger rebuild
    // (the missing file's directory is not auto-watched, so we need to touch a watched file)
    await sleep(1000);
    fs.renameSync(path.join(cwd, 'bar.js'), path.join(cwd, 'foo.js'));
    await editFile(path.join(cwd, 'main.js'), `import { foo } from './foo.js'\nconsole.log(foo)`);
    await waitBuildFinished(watcher);

    const output = path.join(cwd, 'dist', 'main.js');
    expect(fs.readFileSync(output, 'utf-8')).toContain('renamed');
  },
);

// https://github.com/rolldown/rolldown/issues/8892
test.concurrent(
  'watch should emit circular dependency warnings',
  { retry: TEST_RETRY, timeout: TEST_TIMEOUT },
  async ({ task, expect, onTestFinished }) => {
    const retryCount = task.result?.retryCount ?? 0;
    const { dir } = createTestWithMultiFiles('watch-circular-warning', retryCount, {
      'main.js': `import { a } from './a.js'\nconsole.log(a)`,
      'a.js': `import { b } from './b.js'\nexport const a = b`,
      'b.js': `import { a } from './a.js'\nexport const b = a`,
    });
    onTestFinished(() => {
      if (!process.env.CI) {
        fs.rmSync(dir, { recursive: true, force: true });
      }
    });

    const onLogFn = vi.fn();
    const watcher = watch({
      input: path.join(dir, 'main.js'),
      output: { dir: path.join(dir, 'dist') },
      checks: { circularDependency: true },
      plugins: [
        {
          name: 'test-circular-warning',
          onLog(_level, log) {
            if (log.code === 'CIRCULAR_DEPENDENCY') {
              onLogFn();
            }
          },
        },
      ],
    });
    onTestFinished(async () => await watcher.close());

    // Initial build should emit the circular dependency warning
    await waitBuildFinished(watcher);
    expect(onLogFn).toBeCalled();

    // Rebuild should also emit the warning
    onLogFn.mockClear();
    await editFile(path.join(dir, 'a.js'), `import { b } from './b.js'\nexport const a = b + 1`);
    await waitBuildFinished(watcher);
    expect(onLogFn).toBeCalled();
  },
);

test.concurrent(
  'watch should fail when onLog rejects a warning',
  { retry: TEST_RETRY, timeout: TEST_TIMEOUT },
  async ({ task, expect, onTestFinished }) => {
    const retryCount = task.result?.retryCount ?? 0;
    const { dir } = createTestWithMultiFiles('watch-circular-warning-error', retryCount, {
      'main.js': `import { a } from './a.js'\nconsole.log(a)`,
      'a.js': `import { b } from './b.js'\nexport const a = b`,
      'b.js': `import { a } from './a.js'\nexport const b = a`,
    });
    onTestFinished(() => {
      if (!process.env.CI) {
        fs.rmSync(dir, { recursive: true, force: true });
      }
    });

    const watcher = watch({
      input: path.join(dir, 'main.js'),
      output: { dir: path.join(dir, 'dist') },
      checks: { circularDependency: true },
      plugins: [
        {
          name: 'reject-circular-warning',
          onLog(_level, log) {
            if (log.code === 'CIRCULAR_DEPENDENCY') {
              throw new Error('reject circular dependency');
            }
          },
        },
      ],
    });
    onTestFinished(async () => await watcher.close());

    await expect(waitBuildFinished(watcher)).rejects.toThrow('reject circular dependency');
  },
);

// https://github.com/rolldown/rolldown/issues/8912
test.concurrent(
  'watch should not emit false FILE_NAME_CONFLICT on rebuild',
  { retry: TEST_RETRY, timeout: TEST_TIMEOUT },
  async ({ task, expect, onTestFinished }) => {
    const retryCount = task.result?.retryCount ?? 0;
    const { dir } = createTestWithMultiFiles('watch-filename-conflict', retryCount, {
      'main.js': `console.log('hello')`,
    });
    onTestFinished(() => {
      if (!process.env.CI) {
        fs.rmSync(dir, { recursive: true, force: true });
      }
    });

    const filenameConflictFn = vi.fn();
    const watcher = watch({
      input: path.join(dir, 'main.js'),
      output: { dir: path.join(dir, 'dist') },
      onwarn(warning) {
        if (warning.code === 'FILE_NAME_CONFLICT') {
          filenameConflictFn();
        }
      },
      plugins: [
        {
          name: 'emit-asset',
          buildStart() {
            this.emitFile({
              type: 'asset',
              source: 'hello',
              fileName: 'extra.txt',
            });
          },
        },
      ],
    });
    onTestFinished(async () => await watcher.close());

    // Initial build should NOT emit FILE_NAME_CONFLICT
    await waitBuildFinished(watcher);
    expect(filenameConflictFn).not.toBeCalled();

    // Rebuild should also NOT emit FILE_NAME_CONFLICT
    filenameConflictFn.mockClear();
    await editFile(path.join(dir, 'main.js'), `console.log('updated')`);
    await waitBuildFinished(watcher);
    expect(filenameConflictFn).not.toBeCalled();
  },
);

function createTestInputAndOutput(testLabel: string, retryCount: number, content?: string) {
  const uniqueId = crypto.randomUUID().slice(0, 8);
  const dirname = `${testLabel}-${uniqueId}-retry${retryCount}`;
  const dir = path.join(import.meta.dirname, 'temp', dirname);
  fs.mkdirSync(dir, { recursive: true });
  const input = path.join(dir, 'main.js');
  fs.writeFileSync(input, content || 'console.log(1)');
  const outputDir = path.join(dir, 'dist');
  const output = path.join(outputDir, 'main.js');
  return { input, output, dir, outputDir };
}

function createTestWithMultiFiles(
  testLabel: string,
  retryCount: number,
  files: Record<string, string>,
) {
  const uniqueId = crypto.randomUUID().slice(0, 8);
  const dirname = `${testLabel}-${uniqueId}-retry${retryCount}`;
  const dir = path.join(import.meta.dirname, 'temp', dirname);
  fs.mkdirSync(dir, { recursive: true });
  for (const [fileName, content] of Object.entries(files)) {
    fs.writeFileSync(path.join(dir, fileName), content);
  }
  return { dir };
}

async function waitBuildFinished(watcher: RolldownWatcher, updateFn?: () => void) {
  return new Promise<void>((resolve, reject) => {
    let listened = false;
    watcher.on('event', (event) => {
      if (listened) return;

      if (event.code === 'BUNDLE_END') {
        listened = true;
        resolve();
      } else if (event.code === 'ERROR') {
        listened = true;
        reject(event.error);
      }
    });
    updateFn && updateFn();
  });
}

// https://github.com/rolldown/rolldown/issues/8937
test.concurrent(
  'watcher.close() should cancel an in-progress build',
  { retry: TEST_RETRY, timeout: TEST_TIMEOUT },
  async ({ task, expect, onTestFinished }) => {
    const retryCount = task.result?.retryCount ?? 0;
    const { input, output, dir } = createTestInputAndOutput('watch-close-cancel-build', retryCount);
    onTestFinished(() => {
      if (!process.env.CI) {
        fs.rmSync(dir, { recursive: true, force: true });
      }
    });

    const delayPlugin = {
      name: 'delay-plugin',
      async buildStart() {
        await sleep(2000);
      },
    };

    const watcher = watch({
      input,
      output: { file: output },
      plugins: [delayPlugin],
    });

    const events: string[] = [];
    watcher.on('event', (event) => {
      events.push(event.code);
    });

    const closeFn = vi.fn();
    watcher.on('close', closeFn);

    await sleep(500);
    await watcher.close();

    expect(closeFn).toHaveBeenCalled();

    expect(events).not.toContain('BUNDLE_END');
    expect(events).not.toContain('END');

    expect(fs.existsSync(output)).toBe(false);
  },
);
