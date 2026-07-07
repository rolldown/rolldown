// @ts-nocheck This focused unit test intentionally reaches package source outside the test rootDir.
import path from 'node:path';
import { rollup } from 'rollup';
import * as ts from 'typescript';
import { expect, test } from 'vitest';

import { CloseCallbackScope } from '../src/utils/close-callback-scope';
import { normalizePluginOption } from '../src/utils/normalize-plugin-option';

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

test('browser callback-return thenables can await reentrant close without leaking scope', async () => {
  const BrowserCloseCallbackScope = await loadBrowserCloseCallbackScope();
  const scope = new BrowserCloseCallbackScope();
  let resolveFullClose!: () => void;
  const fullClose = new Promise<void>((resolve) => {
    resolveFullClose = resolve;
  });
  const reentrantCloses: Promise<void>[] = [];
  const escapedCloses: Promise<void>[] = [];
  const thenCalls: string[] = [];

  const registerClose = () => {
    const reentrantClose = scope.selectClosePromise(fullClose);
    reentrantCloses.push(reentrantClose);
    queueMicrotask(() => {
      escapedCloses.push(scope.selectClosePromise(fullClose));
    });
    return reentrantClose;
  };
  const innerThenable = {
    // oxlint-disable-next-line unicorn/no-thenable -- exercises nested callback-result assimilation
    then(resolve: (value: string) => void) {
      thenCalls.push('inner');
      const thenClose = registerClose();
      void thenClose.then(() => resolve('settled'));
    },
  };
  const outerThenable = {
    // oxlint-disable-next-line unicorn/no-thenable -- exercises nested callback-result assimilation
    then(resolve: (value: typeof innerThenable) => void) {
      thenCalls.push('outer');
      const reentrantClose = registerClose();
      void reentrantClose.then(() => resolve(innerThenable));
    },
  };

  let fallbackClosed = false;
  const fallback = setTimeout(() => {
    fallbackClosed = true;
    resolveFullClose();
  }, 0);
  const result = await (scope.run(() => outerThenable) as unknown as Promise<string>);
  clearTimeout(fallback);
  resolveFullClose();
  await fullClose;

  expect(result).toBe('settled');
  expect(fallbackClosed).toBe(false);
  expect(thenCalls).toEqual(['outer', 'inner']);
  expect(reentrantCloses).toHaveLength(2);
  expect(reentrantCloses.every((close) => close !== fullClose)).toBe(true);
  expect(escapedCloses).toEqual([fullClose, fullClose]);
});

test.each([
  ['native', async () => CloseCallbackScope],
  ['browser', loadBrowserCloseCallbackScope],
])('%s plugin thenables run inside the close callback scope', async (_, loadScope) => {
  const Scope = await loadScope();
  const scope = new Scope();
  const fullClose = new Promise<void>(() => {});
  let reentrantClose: Promise<void> | undefined;
  const plugin = {
    name: 'thenable-plugin',
  };
  const thenable = {
    // oxlint-disable-next-line unicorn/no-thenable -- exercises plugin thenable normalization
    then(resolve: (value: typeof plugin) => void) {
      reentrantClose = scope.selectClosePromise(fullClose);
      resolve(plugin);
    },
  };

  await expect(normalizePluginOption(thenable, scope)).resolves.toEqual([plugin]);
  expect(reentrantClose).toBeDefined();
  expect(reentrantClose).not.toBe(fullClose);
  await expect(reentrantClose).resolves.toBeUndefined();
});

test.each([
  ['native', async () => CloseCallbackScope],
  ['browser', loadBrowserCloseCallbackScope],
])('%s nested plugin thenables each run inside the close callback scope', async (_, loadScope) => {
  const Scope = await loadScope();
  const scope = new Scope();
  const fullClose = new Promise<void>(() => {});
  const reentrantCloses: Promise<void>[] = [];
  const plugin = {
    name: 'nested-thenable-plugin',
  };
  const innerThenable = {
    // oxlint-disable-next-line unicorn/no-thenable -- exercises nested plugin thenable normalization
    then(resolve: (value: typeof plugin) => void) {
      reentrantCloses.push(scope.selectClosePromise(fullClose));
      resolve(plugin);
    },
  };
  const outerThenable = {
    // oxlint-disable-next-line unicorn/no-thenable -- exercises nested plugin thenable normalization
    then(resolve: (value: typeof innerThenable) => void) {
      reentrantCloses.push(scope.selectClosePromise(fullClose));
      resolve(innerThenable);
    },
  };

  await expect(normalizePluginOption(outerThenable, scope)).resolves.toEqual([plugin]);
  expect(reentrantCloses).toHaveLength(2);
  for (const reentrantClose of reentrantCloses) {
    expect(reentrantClose).not.toBe(fullClose);
    await expect(reentrantClose).resolves.toBeUndefined();
  }
});

test.each([
  ['native', async () => CloseCallbackScope],
  ['browser', loadBrowserCloseCallbackScope],
])('%s callback-return thenables read their then getter once', async (_, loadScope) => {
  const Scope = await loadScope();
  const scope = new Scope();
  let thenReads = 0;
  const secondReadError = new Error('then getter was read twice');
  // oxlint-disable-next-line unicorn/no-thenable -- verifies one-read thenable assimilation
  const thenable = Object.defineProperty({}, 'then', {
    get() {
      thenReads += 1;
      if (thenReads > 1) throw secondReadError;
      return (resolve: (value: string) => void) => resolve('settled');
    },
  });

  await expect(scope.run(() => thenable) as unknown as Promise<string>).resolves.toBe('settled');
  expect(thenReads).toBe(1);
});

test.each([
  ['native', async () => CloseCallbackScope],
  ['browser', loadBrowserCloseCallbackScope],
])(
  '%s callback-return thenables preserve rejection identity and invoke once',
  async (_, loadScope) => {
    const Scope = await loadScope();
    const scope = new Scope();
    const rejection = new TypeError('thenable rejected');
    let callbackCalls = 0;
    let thenCalls = 0;
    const thenable = {
      // oxlint-disable-next-line unicorn/no-thenable -- verifies explicit thenable assimilation
      then(_resolve: (value: unknown) => void, reject: (error: unknown) => void) {
        thenCalls += 1;
        reject(rejection);
        throw new Error('ignored after rejection');
      },
    };

    await expect(
      scope.run(() => {
        callbackCalls += 1;
        return thenable;
      }) as unknown as Promise<unknown>,
    ).rejects.toBe(rejection);
    expect(callbackCalls).toBe(1);
    expect(thenCalls).toBe(1);
  },
);

test.each([
  ['native', async () => CloseCallbackScope],
  ['browser', loadBrowserCloseCallbackScope],
])('%s callback-return thenables reject self-resolution cycles', async (_, loadScope) => {
  const Scope = await loadScope();
  const scope = new Scope();
  let thenCalls = 0;
  const thenable = {
    // oxlint-disable-next-line unicorn/no-thenable -- verifies cyclic thenable rejection
    then(resolve: (value: typeof thenable) => void) {
      thenCalls += 1;
      resolve(thenable);
    },
  };

  await expect(scope.run(() => thenable) as unknown as Promise<unknown>).rejects.toThrow(
    new TypeError('Thenable cycle detected while settling a callback result'),
  );
  expect(thenCalls).toBe(1);
});

test.each([
  ['native', async () => CloseCallbackScope],
  ['browser', loadBrowserCloseCallbackScope],
])('%s callback-return thenables permit mutable self-resolution', async (_, loadScope) => {
  const Scope = await loadScope();
  const scope = new Scope();
  const thenable: { then?: (resolve: (value: unknown) => void) => void } = {
    // oxlint-disable-next-line unicorn/no-thenable -- verifies native mutable-thenable semantics
    then(resolve) {
      delete thenable.then;
      resolve(thenable);
    },
  };

  await expect(scope.run(() => thenable) as unknown as Promise<unknown>).resolves.toBe(thenable);
});

test.each([
  ['native', async () => CloseCallbackScope],
  ['browser', loadBrowserCloseCallbackScope],
])(
  '%s callback-return thenables resolve nested accessors returning undefined',
  async (_, loadScope) => {
    const Scope = await loadScope();
    const scope = new Scope();
    // oxlint-disable-next-line unicorn/no-thenable -- verifies nested accessor assimilation
    const nested = Object.defineProperty({}, 'then', {
      get() {
        return undefined;
      },
    });
    const outer = {
      // oxlint-disable-next-line unicorn/no-thenable -- verifies nested accessor assimilation
      then(resolve: (value: typeof nested) => void) {
        resolve(nested);
      },
    };

    await expect(scope.run(() => outer) as unknown as Promise<unknown>).resolves.toBe(nested);
  },
);

test.each([
  ['native', async () => CloseCallbackScope],
  ['browser', loadBrowserCloseCallbackScope],
])(
  '%s callback-return thenables assimilate nested accessor-returned functions inside the scope',
  async (_, loadScope) => {
    const Scope = await loadScope();
    const scope = new Scope();
    const fullClose = new Promise<void>(() => {});
    let getterCalls = 0;
    let thenCalls = 0;
    let reentrantClose: Promise<void> | undefined;
    // oxlint-disable-next-line unicorn/no-thenable -- verifies nested accessor assimilation
    const nested = Object.defineProperty({}, 'then', {
      get() {
        getterCalls += 1;
        return (resolve: (value: string) => void) => {
          thenCalls += 1;
          reentrantClose = scope.selectClosePromise(fullClose);
          resolve('settled');
        };
      },
    });
    const outer = {
      // oxlint-disable-next-line unicorn/no-thenable -- verifies nested accessor assimilation
      then(resolve: (value: typeof nested) => void) {
        resolve(nested);
      },
    };

    await expect(scope.run(() => outer) as unknown as Promise<string>).resolves.toBe('settled');
    expect(getterCalls).toBe(1);
    expect(thenCalls).toBe(1);
    expect(reentrantClose).toBeDefined();
    expect(reentrantClose).not.toBe(fullClose);
    await expect(reentrantClose).resolves.toBeUndefined();
  },
);

test.each([
  ['native', async () => CloseCallbackScope],
  ['browser', loadBrowserCloseCallbackScope],
])('%s callback-return thenables preserve nested accessor errors', async (_, loadScope) => {
  const Scope = await loadScope();
  const scope = new Scope();
  const getterError = new TypeError('nested then getter failed');
  let getterCalls = 0;
  // oxlint-disable-next-line unicorn/no-thenable -- verifies nested accessor rejection
  const nested = Object.defineProperty({}, 'then', {
    get() {
      getterCalls += 1;
      throw getterError;
    },
  });
  const outer = {
    // oxlint-disable-next-line unicorn/no-thenable -- verifies nested accessor rejection
    then(resolve: (value: typeof nested) => void) {
      resolve(nested);
    },
  };

  await expect(scope.run(() => outer) as unknown as Promise<unknown>).rejects.toBe(getterError);
  expect(getterCalls).toBe(1);
});

test.each([
  ['native', async () => CloseCallbackScope],
  ['browser', loadBrowserCloseCallbackScope],
])('%s callback-return thenables reject mutual resolution cycles', async (_, loadScope) => {
  const Scope = await loadScope();
  const scope = new Scope();
  const first = {
    // oxlint-disable-next-line unicorn/no-thenable -- verifies cyclic thenable rejection
    then(resolve: (value: typeof second) => void) {
      resolve(second);
    },
  };
  const second = {
    // oxlint-disable-next-line unicorn/no-thenable -- verifies cyclic thenable rejection
    then(resolve: (value: typeof first) => void) {
      resolve(first);
    },
  };

  await expect(scope.run(() => first) as unknown as Promise<unknown>).rejects.toThrow(
    new TypeError('Thenable cycle detected while settling a callback result'),
  );
});

test.each([
  ['native', async () => CloseCallbackScope],
  ['browser', loadBrowserCloseCallbackScope],
])('%s plugin normalization rejects a self-resolving thenable', async (_, loadScope) => {
  const Scope = await loadScope();
  const scope = new Scope();
  let thenReads = 0;
  const repeatedReadError = new Error('cycle was assimilated again');
  // oxlint-disable-next-line unicorn/no-thenable -- verifies cyclic thenable rejection
  const thenable = Object.defineProperty({}, 'then', {
    get() {
      thenReads += 1;
      if (thenReads > 1) throw repeatedReadError;
      return (resolve: (value: typeof thenable) => void) => resolve(thenable);
    },
  });

  await expect(normalizePluginOption(thenable, scope)).rejects.toThrow(
    new TypeError('Thenable cycle detected while flattening values'),
  );
  expect(thenReads).toBe(1);
});

test('plugin normalization permits the same thenable in independent sibling branches', async () => {
  const plugins = [{ name: 'first' }, { name: 'second' }];
  let resolutions = 0;
  const thenable = {
    // oxlint-disable-next-line unicorn/no-thenable -- exercises sibling resolution chains
    then(resolve: (value: (typeof plugins)[number]) => void) {
      resolve(plugins[resolutions]);
      resolutions += 1;
    },
  };

  await expect(normalizePluginOption([thenable, thenable])).resolves.toEqual(plugins);
  expect(resolutions).toBe(2);
});

test('plugin normalization rejects circular array graphs without overflowing the stack', async () => {
  const first: unknown[] = [];
  const second = [first];
  first.push(second);

  await expect(normalizePluginOption(first)).rejects.toThrow(
    new TypeError('Array cycle detected while flattening values'),
  );
});

test('plugin normalization permits the same array in independent sibling branches', async () => {
  const shared = [{ name: 'shared' }];

  await expect(normalizePluginOption([shared, shared])).resolves.toEqual([shared[0], shared[0]]);
});

test('plugin array flattening preserves depth-first left-to-right accessor order', async () => {
  const accesses: string[] = [];
  const nested = Object.defineProperties([], {
    0: {
      get() {
        accesses.push('nested');
        return { name: 'nested' };
      },
    },
    length: { value: 1 },
  });
  const plugins = Object.defineProperties([], {
    0: {
      get() {
        accesses.push('first');
        return nested;
      },
    },
    1: {
      get() {
        accesses.push('second');
        return { name: 'second' };
      },
    },
    length: { value: 2 },
  });

  await expect(normalizePluginOption(plugins)).resolves.toEqual([
    { name: 'nested' },
    { name: 'second' },
  ]);
  expect(accesses).toEqual(['first', 'nested', 'second']);
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
