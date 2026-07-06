// @ts-nocheck This focused unit test intentionally reaches package source outside the test rootDir.
import path from 'node:path';
import { rollup } from 'rollup';
import * as ts from 'typescript';
import { expect, test } from 'vitest';

import { CloseCallbackScope } from '../src/utils/close-callback-scope';

test('reentrant close privilege expires when the callback settles', async () => {
  const scope = new CloseCallbackScope();
  let resolveClose!: () => void;
  const fullClose = new Promise<void>((resolve) => {
    resolveClose = resolve;
  });
  let descendantClose: Promise<void> | undefined;
  let descendantRan!: () => void;
  const descendantStarted = new Promise<void>((resolve) => {
    descendantRan = resolve;
  });

  await scope.run(async () => {
    const reentrantClose = scope.selectClosePromise(fullClose);
    expect(reentrantClose).not.toBe(fullClose);
    await reentrantClose;

    setTimeout(() => {
      descendantClose = scope.selectClosePromise(fullClose);
      descendantRan();
    }, 0);
  });

  await descendantStarted;
  expect(descendantClose).toBe(fullClose);

  let settled = false;
  void descendantClose!.then(() => {
    settled = true;
  });
  await Promise.resolve();
  expect(settled).toBe(false);

  resolveClose();
  await expect(descendantClose).resolves.toBeUndefined();
});

test('unrelated concurrent browser close observes cleanup failure', async () => {
  const BrowserCloseCallbackScope = await loadBrowserCloseCallbackScope();
  const scope = new BrowserCloseCallbackScope();
  const pendingClose = new Promise<void>(() => {});
  let callbackClose!: Promise<void>;
  await scope.run(async () => {
    callbackClose = scope.selectClosePromise(pendingClose);
    await callbackClose;
  });
  expect(callbackClose).not.toBe(pendingClose);

  let callbackStarted!: () => void;
  const started = new Promise<void>((resolve) => {
    callbackStarted = resolve;
  });
  let releaseCallback!: () => void;
  const callbackGate = new Promise<void>((resolve) => {
    releaseCallback = resolve;
  });
  const wrappedCallback = scope.wrapCallbacks(async () => {
    callbackStarted();
    await callbackGate;
  });

  const callbackPromise = wrappedCallback();
  await started;

  const cleanupError = new Error('browser cleanup failed');
  let rejectClose!: (error: unknown) => void;
  const fullClose = new Promise<void>((_, reject) => {
    rejectClose = reject;
  });
  const unrelatedClose = scope.selectClosePromise(fullClose);
  let unrelatedCloseSettled = false;
  void unrelatedClose.then(
    () => {
      unrelatedCloseSettled = true;
    },
    () => {
      unrelatedCloseSettled = true;
    },
  );

  await Promise.resolve();
  expect(unrelatedCloseSettled).toBe(false);

  releaseCallback();
  await callbackPromise;
  rejectClose(cleanupError);
  await expect(unrelatedClose).rejects.toBe(cleanupError);
});

let browserCloseCallbackScopePromise: Promise<typeof CloseCallbackScope> | undefined;

function loadBrowserCloseCallbackScope(): Promise<typeof CloseCallbackScope> {
  return (browserCloseCallbackScopePromise ??= buildBrowserCloseCallbackScope());
}

async function buildBrowserCloseCallbackScope(): Promise<typeof CloseCallbackScope> {
  const scopePath = path.resolve(import.meta.dirname, '../src/utils/close-callback-scope.ts');
  const asyncContextPath = path.resolve(import.meta.dirname, '../src/utils/async-context.ts');
  const bundle = await rollup({
    input: scopePath,
    plugins: [
      {
        name: 'browser-close-callback-scope',
        resolveId(id, importer) {
          if (id === 'node:async_hooks') return '\0async-hooks';
          if (importer === scopePath && id === './async-context') {
            return asyncContextPath;
          }
        },
        load(id) {
          if (id === '\0async-hooks') {
            return 'export class AsyncLocalStorage {}';
          }
        },
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
    const output = await bundle.generate({ format: 'esm' });
    const code = output.output.find((item) => item.type === 'chunk')!.code;
    const module = await import(
      `data:text/javascript;base64,${Buffer.from(code).toString('base64')}`
    );
    return module.CloseCallbackScope;
  } finally {
    await bundle.close();
  }
}
