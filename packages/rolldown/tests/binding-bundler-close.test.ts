import assert from 'node:assert/strict';
import { mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';

import '../src/timer-host';
import {
  BindingBundler,
  BindingLogLevel,
  type BindingPluginOptions,
  type BindingResult,
} from '../src/binding.cjs';
import { test } from 'vitest';

type DirectBindingBundler = BindingBundler & {
  closeTerminal(): Promise<BindingResult<void>>;
  waitForFailureClose(): Promise<void>;
};

const BUILD_START = 1 << 0;
const CLOSE_BUNDLE = 1 << 13;

function deferred(): { promise: Promise<void>; resolve: () => void } {
  let resolve!: () => void;
  const promise = new Promise<void>((resolvePromise) => {
    resolve = resolvePromise;
  });
  return { promise, resolve };
}

function settleWithin<T>(promise: Promise<T>, operation: string): Promise<T> {
  const timeoutMs = 5_000;
  let timer: ReturnType<typeof setTimeout> | undefined;
  const timeout = new Promise<never>((_, reject) => {
    timer = setTimeout(() => {
      reject(new Error(`${operation} timed out after ${timeoutMs}ms`));
    }, timeoutMs);
  });
  return Promise.race([promise, timeout]).finally(() => {
    if (timer) clearTimeout(timer);
  });
}

function expectBindingSuccess(result: unknown): void {
  assert.equal((result as { isBindingErrors?: boolean } | undefined)?.isBindingErrors, undefined);
}

test.each(['scan', 'generate', 'write'] as const)(
  'direct BindingBundler close drains an active %s operation before hooks and devtools flush',
  { timeout: 30_000 },
  async (operationName) => {
    const root = mkdtempSync(path.join(tmpdir(), `rolldown-binding-${operationName}-close-`));
    const cwd = path.join(root, 'project');
    const sessionId = `${operationName}-close-session`;
    mkdirSync(cwd, { recursive: true });
    writeFileSync(path.join(cwd, 'main.js'), 'export const value = 1;\n');

    const operationStarted = deferred();
    const releaseOperation = deferred();
    const closeBundleStarted = deferred();
    const releaseCloseBundle = deferred();
    const terminalError = new Error(`${operationName} closeBundle failure`);
    let operationFinished = false;
    let closeBundleCalls = 0;
    const bundler = new BindingBundler() as DirectBindingBundler;
    const options = {
      inputOptions: {
        cwd,
        devtools: { sessionId },
        input: [{ import: './main.js' }],
        logLevel: BindingLogLevel.Silent,
        onLog() {},
        plugins: [
          {
            name: `${operationName}-close-barrier`,
            hookUsage: BUILD_START | CLOSE_BUNDLE,
            async buildStart() {
              operationStarted.resolve();
              await releaseOperation.promise;
              operationFinished = true;
            },
            async closeBundle() {
              closeBundleCalls += 1;
              assert.equal(operationFinished, true);
              closeBundleStarted.resolve();
              await releaseCloseBundle.promise;
              throw terminalError;
            },
          },
        ],
      },
      outputOptions: {
        dir: path.join(cwd, 'dist'),
        plugins: [],
      },
    };

    try {
      const operation = bundler[operationName](options);
      await operationStarted.promise;

      let firstCloseSettled = false;
      const firstClose = bundler
        .close()
        .then(
          () => null,
          (error: unknown) => error,
        )
        .finally(() => {
          firstCloseSettled = true;
        });

      assert.equal(bundler.closed, true);
      await new Promise<void>((resolve) => setImmediate(resolve));
      assert.equal(firstCloseSettled, false);
      assert.equal(closeBundleCalls, 0);
      assert.throws(() => bundler[operationName](options), /closed/i);

      releaseOperation.resolve();
      expectBindingSuccess(await operation);
      await closeBundleStarted.promise;

      let secondCloseSettled = false;
      let terminalCloseSettled = false;
      const secondClose = bundler
        .close()
        .then(
          () => null,
          (error: unknown) => error,
        )
        .finally(() => {
          secondCloseSettled = true;
        });
      const terminalClose = bundler.closeTerminal().finally(() => {
        terminalCloseSettled = true;
      });

      await new Promise<void>((resolve) => setImmediate(resolve));
      assert.equal(firstCloseSettled, false);
      assert.equal(secondCloseSettled, false);
      assert.equal(terminalCloseSettled, false);

      releaseCloseBundle.resolve();
      assert.equal(await firstClose, terminalError);
      assert.equal(await secondClose, terminalError);
      const terminalResult = await terminalClose;
      assert.equal(
        (terminalResult as { errors?: Array<{ field0?: unknown }> }).errors?.[0]?.field0,
        terminalError,
      );

      assert.equal(closeBundleCalls, 1);
      assert.equal(
        await bundler.close().then(
          () => null,
          (error: unknown) => error,
        ),
        terminalError,
      );
      const replayedTerminalResult = await bundler.closeTerminal();
      assert.equal(
        (replayedTerminalResult as { errors?: Array<{ field0?: unknown }> }).errors?.[0]?.field0,
        terminalError,
      );
      assert.equal(closeBundleCalls, 1);

      const logsPath = path.join(cwd, 'node_modules', '.rolldown', sessionId, 'logs.json');
      const logsAfterClose = readFileSync(logsPath, 'utf8');
      assert.match(logsAfterClose, /"action":"BuildEnd"/);
      await new Promise<void>((resolve) => setImmediate(resolve));
      assert.equal(readFileSync(logsPath, 'utf8'), logsAfterClose);
    } finally {
      releaseOperation.resolve();
      releaseCloseBundle.resolve();
      await bundler.closeTerminal().catch(() => {});
      rmSync(root, { force: true, recursive: true });
    }
  },
);

test(
  'failure-triggered close waits for concurrent output operations',
  { timeout: 30_000 },
  async () => {
    const root = mkdtempSync(path.join(tmpdir(), 'rolldown-binding-failure-close-barrier-'));
    const cwd = path.join(root, 'project');
    mkdirSync(cwd, { recursive: true });
    writeFileSync(path.join(cwd, 'main.js'), 'export const value = 1;\n');

    const firstStarted = deferred();
    const releaseFirst = deferred();
    let firstFinished = false;
    const closeObservations: boolean[] = [];
    const bundler = new BindingBundler() as DirectBindingBundler;
    const options = (plugin: BindingPluginOptions) => ({
      inputOptions: {
        cwd,
        input: [{ import: './main.js' }],
        logLevel: BindingLogLevel.Silent,
        onLog() {},
        plugins: [plugin],
      },
      outputOptions: {
        dir: path.join(cwd, 'dist'),
        plugins: [],
      },
    });

    try {
      const first = bundler.generate(
        options({
          name: 'gated-concurrent-output',
          hookUsage: BUILD_START,
          async buildStart() {
            firstStarted.resolve();
            await releaseFirst.promise;
            firstFinished = true;
          },
        }),
      );
      await firstStarted.promise;

      const failedOutput = bundler.generate(
        options({
          name: 'failed-concurrent-output',
          hookUsage: BUILD_START | CLOSE_BUNDLE,
          buildStart() {
            throw new Error('injected concurrent output failure');
          },
          closeBundle() {
            closeObservations.push(firstFinished);
          },
        }),
      );

      const failedResult = await settleWithin(
        failedOutput,
        'failed output promise before concurrent output drain',
      );
      assert.equal((failedResult as { isBindingErrors?: boolean }).isBindingErrors, true);
      assert.deepEqual(closeObservations, []);

      releaseFirst.resolve();
      expectBindingSuccess(await first);
      await settleWithin(
        bundler.waitForFailureClose(),
        'failure-triggered close after concurrent output drain',
      );
      assert.deepEqual(closeObservations, [true]);
    } finally {
      releaseFirst.resolve();
      await bundler.closeTerminal().catch(() => {});
      rmSync(root, { force: true, recursive: true });
    }
  },
);

test(
  'nested failed generate settles before its outer operation and retains closeBundle failure',
  { timeout: 30_000 },
  async () => {
    const root = mkdtempSync(path.join(tmpdir(), 'rolldown-binding-nested-failure-close-'));
    const cwd = path.join(root, 'project');
    mkdirSync(cwd, { recursive: true });
    writeFileSync(path.join(cwd, 'main.js'), 'export const value = 1;\n');

    const nestedBuildError = new Error('nested output build failure');
    const nestedCloseError = new TypeError('nested output closeBundle failure');
    const nestedCloseFinished = deferred();
    let outerContinued = false;
    let nestedCloseCalls = 0;
    const bundler = new BindingBundler() as DirectBindingBundler;
    const options = (plugin: BindingPluginOptions) => ({
      inputOptions: {
        cwd,
        input: [{ import: './main.js' }],
        logLevel: BindingLogLevel.Silent,
        onLog() {},
        plugins: [plugin],
      },
      outputOptions: {
        dir: path.join(cwd, 'dist'),
        plugins: [],
      },
    });

    try {
      const outerResult = await settleWithin(
        bundler.generate(
          options({
            name: 'outer-output-awaiting-nested-failure',
            hookUsage: BUILD_START,
            async buildStart() {
              const nestedResult = await bundler.generate(
                options({
                  name: 'nested-failed-output',
                  hookUsage: BUILD_START | CLOSE_BUNDLE,
                  buildStart() {
                    throw nestedBuildError;
                  },
                  closeBundle() {
                    nestedCloseCalls += 1;
                    nestedCloseFinished.resolve();
                    throw nestedCloseError;
                  },
                }),
              );
              assert.equal((nestedResult as { isBindingErrors?: boolean }).isBindingErrors, true);
              outerContinued = true;
            },
          }),
        ),
        'outer generate awaiting nested failed generate',
      );

      expectBindingSuccess(outerResult);
      assert.equal(outerContinued, true);
      await settleWithin(nestedCloseFinished.promise, 'nested failure closeBundle');
      await settleWithin(bundler.waitForFailureClose(), 'nested failure close completion');

      const closeResult = await settleWithin(
        bundler.closeTerminal(),
        'terminal close after nested failed generate',
      );
      assert.deepEqual(
        (closeResult as { errors?: Array<{ field0?: unknown }> }).errors?.map(
          (error) => error.field0,
        ),
        [nestedCloseError],
      );
      assert.equal(nestedCloseCalls, 1);
    } finally {
      await bundler.closeTerminal().catch(() => {});
      rmSync(root, { force: true, recursive: true });
    }
  },
);

test(
  'failure-triggered close blocks new output entry until closeBundle settles',
  { timeout: 30_000 },
  async () => {
    const root = mkdtempSync(path.join(tmpdir(), 'rolldown-binding-failure-close-entry-gate-'));
    const cwd = path.join(root, 'project');
    mkdirSync(cwd, { recursive: true });
    writeFileSync(path.join(cwd, 'main.js'), 'export const value = 1;\n');

    const closeBundleStarted = deferred();
    const releaseCloseBundle = deferred();
    const bundler = new BindingBundler() as DirectBindingBundler;
    const options = (plugin: BindingPluginOptions) => ({
      inputOptions: {
        cwd,
        input: [{ import: './main.js' }],
        logLevel: BindingLogLevel.Silent,
        onLog() {},
        plugins: [plugin],
      },
      outputOptions: {
        dir: path.join(cwd, 'dist'),
        plugins: [],
      },
    });

    try {
      const failedOutput = bundler.generate(
        options({
          name: 'failed-output-with-gated-close',
          hookUsage: BUILD_START | CLOSE_BUNDLE,
          buildStart() {
            throw new Error('injected output failure');
          },
          async closeBundle() {
            closeBundleStarted.resolve();
            await releaseCloseBundle.promise;
          },
        }),
      );
      await closeBundleStarted.promise;

      assert.throws(
        () =>
          bundler.generate(
            options({
              name: 'output-entering-during-close',
              hookUsage: 0,
            }),
          ),
        /Cannot start a new output while closeBundle is still running/,
      );

      releaseCloseBundle.resolve();
      assert.equal(((await failedOutput) as { isBindingErrors?: boolean }).isBindingErrors, true);
      await settleWithin(bundler.waitForFailureClose(), 'gated failure close completion');

      const laterOutput = await bundler.generate(
        options({
          name: 'output-after-failure-close',
          hookUsage: 0,
        }),
      );
      expectBindingSuccess(laterOutput);
    } finally {
      releaseCloseBundle.resolve();
      await bundler.closeTerminal().catch(() => {});
      rmSync(root, { force: true, recursive: true });
    }
  },
);

test(
  'explicit close racing a failed output preserves its closeBundle diagnostic',
  { timeout: 30_000 },
  async () => {
    const root = mkdtempSync(path.join(tmpdir(), 'rolldown-binding-failure-close-diagnostic-'));
    const cwd = path.join(root, 'project');
    mkdirSync(cwd, { recursive: true });
    writeFileSync(path.join(cwd, 'main.js'), 'export const value = 1;\n');

    const siblingStarted = deferred();
    const releaseSibling = deferred();
    const failureStarted = deferred();
    const failure = new TypeError('injected diagnostic-preservation failure');
    let receivedCloseError: unknown;
    let closeBundleCalls = 0;
    const bundler = new BindingBundler() as DirectBindingBundler;
    const options = (plugin: BindingPluginOptions) => ({
      inputOptions: {
        cwd,
        input: [{ import: './main.js' }],
        logLevel: BindingLogLevel.Silent,
        onLog() {},
        plugins: [plugin],
      },
      outputOptions: {
        dir: path.join(cwd, 'dist'),
        plugins: [],
      },
    });

    try {
      const sibling = bundler.generate(
        options({
          name: 'gated-sibling-before-explicit-close',
          hookUsage: BUILD_START,
          async buildStart() {
            siblingStarted.resolve();
            await releaseSibling.promise;
          },
        }),
      );
      await siblingStarted.promise;

      const failedOutput = bundler.generate(
        options({
          name: 'failed-output-racing-explicit-close',
          hookUsage: BUILD_START | CLOSE_BUNDLE,
          buildStart() {
            failureStarted.resolve();
            throw failure;
          },
          closeBundle(_ctx, error) {
            closeBundleCalls += 1;
            receivedCloseError = error;
          },
        }),
      );
      await failureStarted.promise;

      const close = bundler.closeTerminal();
      releaseSibling.resolve();

      expectBindingSuccess(await sibling);
      assert.equal(((await failedOutput) as { isBindingErrors?: boolean }).isBindingErrors, true);
      expectBindingSuccess(await close);
      assert.equal(closeBundleCalls, 1);
      assert.deepEqual(
        (receivedCloseError as Array<{ field0?: unknown }>).map((error) => error.field0),
        [failure],
      );
    } finally {
      releaseSibling.resolve();
      await bundler.closeTerminal().catch(() => {});
      rmSync(root, { force: true, recursive: true });
    }
  },
);

test(
  'final close merges an older failure-triggered closeBundle failure with the latest output',
  { timeout: 30_000 },
  async () => {
    const root = mkdtempSync(path.join(tmpdir(), 'rolldown-binding-retained-close-failure-'));
    const cwd = path.join(root, 'project');
    mkdirSync(cwd, { recursive: true });
    writeFileSync(path.join(cwd, 'main.js'), 'export const value = 1;\n');

    const olderCloseError = new TypeError('older output closeBundle failure');
    const latestCloseError = new RangeError('latest output closeBundle failure');
    let olderCloseCalls = 0;
    let latestCloseCalls = 0;
    const bundler = new BindingBundler() as DirectBindingBundler;
    const options = (plugin: BindingPluginOptions) => ({
      inputOptions: {
        cwd,
        input: [{ import: './main.js' }],
        logLevel: BindingLogLevel.Silent,
        onLog() {},
        plugins: [plugin],
      },
      outputOptions: {
        dir: path.join(cwd, 'dist'),
        plugins: [],
      },
    });

    try {
      const failedOutput = await bundler.generate(
        options({
          name: 'older-failed-output',
          hookUsage: BUILD_START | CLOSE_BUNDLE,
          buildStart() {
            throw new Error('older output build failure');
          },
          closeBundle() {
            olderCloseCalls += 1;
            throw olderCloseError;
          },
        }),
      );
      assert.equal((failedOutput as { isBindingErrors?: boolean }).isBindingErrors, true);
      await settleWithin(bundler.waitForFailureClose(), 'older failure close completion');

      expectBindingSuccess(
        await bundler.generate(
          options({
            name: 'latest-successful-output',
            hookUsage: CLOSE_BUNDLE,
            closeBundle() {
              latestCloseCalls += 1;
              throw latestCloseError;
            },
          }),
        ),
      );

      const closeResult = await bundler.closeTerminal();
      assert.deepEqual(
        (closeResult as { errors?: Array<{ field0?: unknown }> }).errors?.map(
          (error) => error.field0,
        ),
        [olderCloseError, latestCloseError],
      );
      assert.equal(olderCloseCalls, 1);
      assert.equal(latestCloseCalls, 1);

      const replayedCloseResult = await bundler.closeTerminal();
      assert.deepEqual(
        (replayedCloseResult as { errors?: Array<{ field0?: unknown }> }).errors?.map(
          (error) => error.field0,
        ),
        [olderCloseError, latestCloseError],
      );
      assert.equal(olderCloseCalls, 1);
      assert.equal(latestCloseCalls, 1);
    } finally {
      await bundler.closeTerminal().catch(() => {});
      rmSync(root, { force: true, recursive: true });
    }
  },
);
