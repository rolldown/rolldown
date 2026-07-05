import path from 'node:path';

import { chromium } from 'playwright-chromium';
import { rollup } from 'rollup';
import * as ts from 'typescript';
import { expect, test } from 'vitest';

const TEST_TIMEOUT = 30_000;

test('browser watcher scheduling and close lifecycle', { timeout: TEST_TIMEOUT }, async () => {
  const code = await buildBrowserWatcherHarness();
  const browser = await chromium.launch({ headless: true });
  try {
    const page = await browser.newPage();
    await page.addScriptTag({ content: code });
    const result = await page.evaluate(async () => {
      return (
        globalThis as typeof globalThis & {
          runBrowserWatcherTests(): Promise<{
            cancellation: Record<string, number | boolean>;
            cleanup: Record<string, number>;
            lifecycle: Record<string, number | boolean>;
          }>;
        }
      ).runBrowserWatcherTests();
    });

    expect(result.lifecycle).toEqual({
      concurrentCloseDuringListenerSettled: true,
      initiatingCloseSettledBeforeListenerFinished: false,
      leaseReleaseCalls: 1,
      nativeCloseCalls: 1,
      reentrantCloseSettled: true,
      runCallsAfterHostTurn: 1,
      runCallsBeforeHostTurn: 0,
      stopWorkerCalls: 1,
    });
    expect(result.cancellation).toEqual({
      closeRejectedWithCancellationError: true,
      leaseReleaseCalls: 1,
      nativeCloseCalls: 1,
      runCalls: 0,
      stopWorkerCalls: 1,
    });
    expect(result.cleanup).toEqual({
      closeEvents: 1,
      leaseReleaseCalls: 3,
      nativeCloseCalls: 1,
      runCalls: 0,
      stopWorkerCalls: 3,
    });
  } finally {
    await browser.close();
  }
});

async function buildBrowserWatcherHarness(): Promise<string> {
  const watcherPath = path.resolve(import.meta.dirname, '../../src/api/watch/watcher.ts');
  const emitterPath = path.resolve(import.meta.dirname, '../../src/api/watch/watch-emitter.ts');
  const asyncContextPath = path.resolve(import.meta.dirname, '../../src/utils/async-context.ts');
  const virtualModules = new Map<string, string>([
    [
      'binding',
      `
        export class BindingWatcher {
          constructor(_options, callback) {
            this.callback = callback;
            globalThis.__watchHarness.bindingConstructed += 1;
          }
          async run() {
            globalThis.__watchHarness.runCalls += 1;
          }
          waitForClose() {
            globalThis.__watchHarness.waitForCloseCalls += 1;
          }
          async close() {
            globalThis.__watchHarness.nativeCloseCalls += 1;
            await Promise.resolve();
            await this.callback({ eventKind: () => 'close' });
          }
        }
      `,
    ],
    [
      'runtime-lifecycle',
      `
        export class CloseCoordinator {
          constructor(message) {
            this.message = message;
          }
          close(attempt) {
            return (this.promise ??= this.run(attempt));
          }
          async run(attempt) {
            const result = await attempt();
            if (result.retryable) this.promise = undefined;
            if (result.errors.length === 1) throw result.errors[0];
            if (result.errors.length > 1) {
              throw new AggregateError(result.errors, this.message);
            }
          }
        }

        export function acquireRuntimeLease() {
          const harness = globalThis.__watchHarness;
          let released = false;
          return {
            release() {
              if (released) return;
              harness.leaseReleaseCalls += 1;
              if (harness.leaseReleaseFailures > 0) {
                harness.leaseReleaseFailures -= 1;
                throw harness.leaseReleaseError;
              }
              released = true;
            },
          };
        }
      `,
    ],
    [
      'create-bundler-option',
      `
        export async function createBundlerOptions(inputOptions) {
          const harness = globalThis.__watchHarness;
          let stopped = false;
          return {
            bundlerOptions: {},
            inputOptions: { ...inputOptions, watch: inputOptions.watch ?? null },
            onLog() {},
            async stopWorkers() {
              if (stopped) return;
              harness.stopWorkerCalls += 1;
              if (harness.stopWorkerFailures > 0) {
                harness.stopWorkerFailures -= 1;
                throw harness.stopWorkerError;
              }
              stopped = true;
            },
          };
        }
      `,
    ],
    [
      'plugin-driver',
      `
        export const PluginDriver = {
          async callOptionsHook(option) {
            return option;
          },
        };
      `,
    ],
    ['logging', `export const LOG_LEVEL_WARN = 'warn';`],
    ['logs', `export function logMultipleWatcherOption() { return {}; }`],
    ['error', `export function aggregateBindingErrorsIntoJsError(error) { return error; }`],
    ['misc', `export function arraify(value) { return Array.isArray(value) ? value : [value]; }`],
    ['async-hooks', `export class AsyncLocalStorage {}`],
  ]);

  const bundle = await rollup({
    input: 'browser-watcher-harness',
    plugins: [
      {
        name: 'browser-watcher-harness',
        resolveId(id, importer) {
          if (id === 'browser-watcher-harness') return `\0${id}`;
          if (id === 'node:async_hooks') return '\0async-hooks';
          if (!importer) return;
          if (id === '../../utils/async-context') return asyncContextPath;
          if (id === '../../binding.cjs') return '\0binding';
          if (id === '../../runtime-lifecycle') return '\0runtime-lifecycle';
          if (id === '../../utils/create-bundler-option') return '\0create-bundler-option';
          if (id === '../../plugin/plugin-driver') return '\0plugin-driver';
          if (id === '../../log/logging') return '\0logging';
          if (id === '../../log/logs') return '\0logs';
          if (id === '../../utils/error') return '\0error';
          if (id === '../../utils/misc') return '\0misc';
        },
        load(id) {
          if (id === '\0browser-watcher-harness') {
            return browserHarnessEntry(watcherPath, emitterPath);
          }
          return virtualModules.get(id.slice(1));
        },
      },
      {
        name: 'transpile-browser-watcher-harness',
        transform(code, id) {
          if (!id.endsWith('.ts')) return;
          return {
            code: ts.transpileModule(code.replaceAll('import.meta.browserBuild', 'true'), {
              compilerOptions: {
                module: ts.ModuleKind.ESNext,
                target: ts.ScriptTarget.ES2022,
              },
              fileName: id,
            }).outputText,
            map: null,
          };
        },
      },
    ],
  });

  try {
    const output = await bundle.generate({ format: 'iife', name: 'BrowserWatcherHarness' });
    return output.output.find((item) => item.type === 'chunk')!.code;
  } finally {
    await bundle.close();
  }
}

function browserHarnessEntry(watcherPath: string, emitterPath: string): string {
  return `
    import { createWatcher } from ${JSON.stringify(watcherPath)};
    import { WatcherEmitter } from ${JSON.stringify(emitterPath)};

    function resetHarness() {
      globalThis.__watchHarness = {
        bindingConstructed: 0,
        leaseReleaseCalls: 0,
        leaseReleaseError: new Error('lease release failed'),
        leaseReleaseFailures: 0,
        nativeCloseCalls: 0,
        runCalls: 0,
        stopWorkerCalls: 0,
        stopWorkerError: new Error('worker stop failed'),
        stopWorkerFailures: 0,
        waitForCloseCalls: 0,
      };
      return globalThis.__watchHarness;
    }

    async function withTimeout(callback) {
      return Promise.race([
        callback(),
        new Promise((_, reject) => {
          globalThis.setTimeout(
            () => reject(new Error('browser watcher regression timed out')),
            5_000,
          );
        }),
      ]);
    }

    globalThis.runBrowserWatcherTests = () => withTimeout(async () => {
      const lifecycleHarness = resetHarness();
      const emitter = new WatcherEmitter();
      await createWatcher(emitter, { output: {} });
      const runCallsBeforeHostTurn = lifecycleHarness.runCalls;
      await new Promise((resolve) => globalThis.setTimeout(resolve, 0));
      const runCallsAfterHostTurn = lifecycleHarness.runCalls;

      let releaseCloseListener;
      const closeListenerGate = new Promise((resolve) => {
        releaseCloseListener = resolve;
      });
      let markReentrantCloseSettled;
      const reentrantCloseSettled = new Promise((resolve) => {
        markReentrantCloseSettled = resolve;
      });
      emitter.on('close', async () => {
        await Promise.resolve();
        await emitter.close();
        markReentrantCloseSettled();
        await closeListenerGate;
      });

      let initiatingCloseSettled = false;
      const firstClose = emitter.close().finally(() => {
        initiatingCloseSettled = true;
      });
      await reentrantCloseSettled;
      let concurrentCloseDuringListenerSettled = false;
      const concurrentCloseDuringListener = emitter.close().finally(() => {
        concurrentCloseDuringListenerSettled = true;
      });
      await Promise.resolve();
      await new Promise((resolve) => globalThis.setTimeout(resolve, 0));
      const initiatingCloseSettledBeforeListenerFinished = initiatingCloseSettled;
      releaseCloseListener();
      await Promise.all([firstClose, concurrentCloseDuringListener]);

      const lifecycle = {
        concurrentCloseDuringListenerSettled,
        initiatingCloseSettledBeforeListenerFinished,
        leaseReleaseCalls: lifecycleHarness.leaseReleaseCalls,
        nativeCloseCalls: lifecycleHarness.nativeCloseCalls,
        reentrantCloseSettled: true,
        runCallsAfterHostTurn,
        runCallsBeforeHostTurn,
        stopWorkerCalls: lifecycleHarness.stopWorkerCalls,
      };

      const cancellationHarness = resetHarness();
      const cancellationEmitter = new WatcherEmitter();
      await createWatcher(cancellationEmitter, { output: {} });
      const cancellationError = new Error('host turn cancellation failed');
      const originalClearTimeout = globalThis.clearTimeout;
      globalThis.clearTimeout = () => {
        throw cancellationError;
      };
      const cancellationResult = await Promise.allSettled([cancellationEmitter.close()]);
      globalThis.clearTimeout = originalClearTimeout;
      await new Promise((resolve) => globalThis.setTimeout(resolve, 0));
      const cancellation = {
        closeRejectedWithCancellationError:
          cancellationResult[0].status === 'rejected' &&
          cancellationResult[0].reason === cancellationError,
        leaseReleaseCalls: cancellationHarness.leaseReleaseCalls,
        nativeCloseCalls: cancellationHarness.nativeCloseCalls,
        runCalls: cancellationHarness.runCalls,
        stopWorkerCalls: cancellationHarness.stopWorkerCalls,
      };

      const cleanupHarness = resetHarness();
      cleanupHarness.leaseReleaseFailures = 2;
      cleanupHarness.stopWorkerFailures = 2;
      const cleanupEmitter = new WatcherEmitter();
      let cleanupCloseEvents = 0;
      cleanupEmitter.on('close', () => {
        cleanupCloseEvents += 1;
      });
      const originalSetTimeout = globalThis.setTimeout;
      globalThis.setTimeout = () => {
        throw new Error('host turn scheduling failed');
      };
      let cleanupSetupError;
      try {
        await createWatcher(cleanupEmitter, { output: {} });
        throw new Error('watcher creation unexpectedly succeeded');
      } catch (error) {
        const errors = error instanceof AggregateError ? error.errors : [error];
        if (!errors.some((item) => String(item).includes('host turn scheduling failed'))) {
          throw error;
        }
        cleanupSetupError = error;
      } finally {
        globalThis.setTimeout = originalSetTimeout;
      }
      await cleanupEmitter.failSetup(cleanupSetupError);
      await cleanupEmitter.close();

      return {
        cancellation,
        cleanup: {
          closeEvents: cleanupCloseEvents,
          leaseReleaseCalls: cleanupHarness.leaseReleaseCalls,
          nativeCloseCalls: cleanupHarness.nativeCloseCalls,
          runCalls: cleanupHarness.runCalls,
          stopWorkerCalls: cleanupHarness.stopWorkerCalls,
        },
        lifecycle,
      };
    });
  `;
}
