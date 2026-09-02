// @ts-nocheck This focused unit test intentionally reaches package source outside the test rootDir.
import { AsyncLocalStorage } from 'node:async_hooks';
import path from 'node:path';
import { rolldown } from 'rolldown';
import type { Plugin } from 'rolldown';
import { rollup } from 'rollup';
import * as ts from 'typescript';
import { expect, test } from 'vitest';

import type { configureAsyncContext } from '../src/utils/async-context';
import { CloseCallbackScope } from '../src/utils/close-callback-scope';
import { normalizePluginOption } from '../src/utils/normalize-plugin-option';

test.each([
  ['native', async () => CloseCallbackScope],
  ['browser', loadBrowserCloseCallbackScope],
])('%s close identities only acknowledge the matching callback', async (_, loadScope) => {
  const Scope = await loadScope();
  const scope = new Scope();
  const matchingClose = new Promise<void>(() => {});
  const differentError = new Error('different close failed');
  let rejectDifferentClose!: (error: unknown) => void;
  const differentClose = new Promise<void>((_, reject) => {
    rejectDifferentClose = reject;
  });

  const wrappedCallback = scope.wrapCallbacks(() => [
    scope.selectClosePromise(matchingClose, 'matching'),
    scope.selectClosePromise(differentClose, 'different'),
  ]);
  const [selectedMatchingClose, selectedDifferentClose] = await scope.runWithCloseIdentity(
    'matching',
    async () => {
      await Promise.resolve();
      return wrappedCallback();
    },
  );

  expect(selectedMatchingClose).not.toBe(matchingClose);
  await expect(selectedMatchingClose).resolves.toBeUndefined();
  expect(selectedDifferentClose).toBeInstanceOf(Promise);

  rejectDifferentClose(differentError);
  await expect(selectedDifferentClose).rejects.toBe(differentError);
});

test.each([
  ['native', async () => CloseCallbackScope],
  ['browser', loadBrowserCloseCallbackScope],
])('%s identity-less closes remain scoped to their owner', async (_, loadScope) => {
  const Scope = await loadScope();
  const scopeA = new Scope();
  const scopeB = new Scope();
  const closeError = new Error('different scope close failed');
  let rejectClose!: (error: unknown) => void;
  const closePromise = new Promise<void>((_, reject) => {
    rejectClose = reject;
  });
  let selectedClose!: Promise<void>;

  scopeA.run(() => {
    selectedClose = scopeB.selectClosePromise(closePromise);
  });

  expect(selectedClose).toBe(closePromise);
  rejectClose(closeError);
  await expect(selectedClose).rejects.toBe(closeError);
});

test.each([
  ['native', async () => CloseCallbackScope],
  ['browser', loadBrowserCloseCallbackScope],
])('%s nested close identities acknowledge an active ancestor', async (_, loadScope) => {
  const Scope = await loadScope();
  const scope = new Scope();
  const ancestorClose = new Promise<void>(() => {});
  const unrelatedError = new Error('unrelated close failed');
  let rejectUnrelatedClose!: (error: unknown) => void;
  const unrelatedClose = new Promise<void>((_, reject) => {
    rejectUnrelatedClose = reject;
  });
  let selectedAncestorClose!: Promise<void>;
  let selectedUnrelatedClose!: Promise<void>;

  await scope.runWithCloseIdentity('ancestor', async () => {
    await Promise.resolve();
    await scope.runWithCloseIdentity('nested', async () => {
      await Promise.resolve();
      selectedAncestorClose = scope.selectClosePromise(ancestorClose, 'ancestor');
      selectedUnrelatedClose = scope.selectClosePromise(unrelatedClose, 'unrelated');
    });
  });

  expect(selectedAncestorClose).not.toBe(ancestorClose);
  await expect(selectedAncestorClose).resolves.toBeUndefined();
  expect(selectedUnrelatedClose).toBeInstanceOf(Promise);

  rejectUnrelatedClose(unrelatedError);
  await expect(selectedUnrelatedClose).rejects.toBe(unrelatedError);
});

test.each([
  ['native', async () => CloseCallbackScope],
  ['browser', loadBrowserCloseCallbackScope],
])(
  '%s close dependencies detect a cycle across separate callback contexts',
  async (_, loadScope) => {
    const Scope = await loadScope();
    const scope = new Scope();
    let resolveCloseB!: () => void;
    const closeB = new Promise<void>((resolve) => {
      resolveCloseB = resolve;
    });
    const closeA = new Promise<void>(() => {});
    let markAWaitingForB!: () => void;
    const aWaitingForB = new Promise<void>((resolve) => {
      markAWaitingForB = resolve;
    });
    let selectedB!: Promise<void>;

    const callbackA = scope.runWithCloseIdentity('A', async () => {
      selectedB = scope.selectClosePromise(closeB, 'B');
      markAWaitingForB();
      await selectedB;
    });
    await aWaitingForB;

    let selectedA!: Promise<void>;
    await scope.runWithCloseIdentity('B', async () => {
      selectedA = scope.selectClosePromise(closeA, 'A');
      await selectedA;
      resolveCloseB();
    });
    await callbackA;

    expect(selectedB).toBeInstanceOf(Promise);
    expect(selectedA).not.toBe(closeA);
    await expect(selectedA).resolves.toBeUndefined();
  },
);

test.each([
  ['native', async () => CloseCallbackScope],
  ['browser', loadBrowserCloseCallbackScope],
])('%s close dependencies detect a cycle across separate scopes', async (_, loadScope) => {
  const Scope = await loadScope();
  const scopeA = new Scope();
  const scopeB = new Scope();
  let resolveCloseB!: () => void;
  const closeB = new Promise<void>((resolve) => {
    resolveCloseB = resolve;
  });
  const closeA = new Promise<void>(() => {});
  let markAWaitingForB!: () => void;
  const aWaitingForB = new Promise<void>((resolve) => {
    markAWaitingForB = resolve;
  });

  const callbackA = scopeA.runWithCloseIdentity('cross-scope-A', async () => {
    markAWaitingForB();
    await scopeB.selectClosePromise(closeB, 'cross-scope-B');
  });
  await aWaitingForB;

  let selectedA!: Promise<void>;
  await scopeB.runWithCloseIdentity('cross-scope-B', async () => {
    selectedA = scopeA.selectClosePromise(closeA, 'cross-scope-A');
    await selectedA;
    resolveCloseB();
  });
  await callbackA;

  expect(selectedA).not.toBe(closeA);
  await expect(selectedA).resolves.toBeUndefined();
});

test.each([
  ['native', async () => CloseCallbackScope],
  ['browser', loadBrowserCloseCallbackScope],
])('%s close dependencies expire with their source invocation', async (_, loadScope) => {
  const Scope = await loadScope();
  const scopeA = new Scope();
  const scopeB = new Scope();
  const closeA = new Promise<void>(() => {});
  const closeB = new Promise<void>(() => {});

  await scopeA.runWithCloseIdentity('expired-source-A', async () => {
    scopeB.selectClosePromise(closeB, 'expired-source-B');
  });

  let selectedA!: Promise<void>;
  scopeB.runWithCloseIdentity('expired-source-B', () => {
    selectedA = scopeA.selectClosePromise(closeA, 'expired-source-A');
  });

  expect(selectedA).toBeInstanceOf(Promise);
});

test('fire-and-forget closes do not create dependency-cycle acknowledgements', async () => {
  const scope = new CloseCallbackScope();
  const closeAError = new Error('source close failed');
  let rejectCloseA!: (error: unknown) => void;
  const closeA = new Promise<void>((_, reject) => {
    rejectCloseA = reject;
  });
  const closeB = new Promise<void>(() => {});
  let releaseCallbackA!: () => void;
  const callbackAGate = new Promise<void>((resolve) => {
    releaseCallbackA = resolve;
  });
  let markCallbackAStarted!: () => void;
  const callbackAStarted = new Promise<void>((resolve) => {
    markCallbackAStarted = resolve;
  });

  const callbackA = scope.runWithCloseIdentity('fire-and-forget-A', async () => {
    void scope.selectClosePromise(closeB, 'fire-and-forget-B');
    markCallbackAStarted();
    await callbackAGate;
  });
  await callbackAStarted;

  let selectedA!: Promise<void>;
  const callbackB = scope.runWithCloseIdentity('fire-and-forget-B', async () => {
    selectedA = scope.selectClosePromise(closeA, 'fire-and-forget-A');
    await selectedA;
  });
  rejectCloseA(closeAError);
  await expect(callbackB).rejects.toBe(closeAError);
  await expect(selectedA).rejects.toBe(closeAError);

  releaseCallbackA();
  await callbackA;
});

test.each([
  ['native', async () => CloseCallbackScope],
  ['browser', loadBrowserCloseCallbackScope],
])(
  '%s dependency-aware close promises are stable native Promise instances',
  async (_, loadScope) => {
    const Scope = await loadScope();
    const scope = new Scope();
    const closePromise = Promise.resolve();
    const first = scope.selectClosePromise(closePromise, 'stable-close');
    const second = scope.selectClosePromise(closePromise, 'stable-close');

    expect(first).toBe(second);
    expect(first).toBeInstanceOf(Promise);
    await first.finally(() => {});
  },
);

test('browser close identities remain active until every matching callback settles', async () => {
  const BrowserCloseCallbackScope = await loadBrowserCloseCallbackScope();
  const scope = new BrowserCloseCallbackScope();
  const fullClose = new Promise<void>(() => {});
  let releaseFirst!: () => void;
  const firstGate = new Promise<void>((resolve) => {
    releaseFirst = resolve;
  });
  let releaseSecond!: () => void;
  const secondGate = new Promise<void>((resolve) => {
    releaseSecond = resolve;
  });
  let markFirstStarted!: () => void;
  const firstStarted = new Promise<void>((resolve) => {
    markFirstStarted = resolve;
  });
  let markSecondStarted!: () => void;
  const secondStarted = new Promise<void>((resolve) => {
    markSecondStarted = resolve;
  });

  const firstCallback = scope.runWithCloseIdentity('matching', async () => {
    await Promise.resolve();
    markFirstStarted();
    await firstGate;
  });
  const secondCallback = scope.runWithCloseIdentity('matching', async () => {
    await Promise.resolve();
    markSecondStarted();
    await secondGate;
  });
  await Promise.all([firstStarted, secondStarted]);

  const whileBothActive = scope.selectClosePromise(fullClose, 'matching');
  expect(whileBothActive).not.toBe(fullClose);
  await expect(whileBothActive).resolves.toBeUndefined();

  releaseFirst();
  await firstCallback;
  const whileSecondActive = scope.selectClosePromise(fullClose, 'matching');
  expect(whileSecondActive).not.toBe(fullClose);
  await expect(whileSecondActive).resolves.toBeUndefined();

  releaseSecond();
  await secondCallback;
  const afterCallbacks = scope.selectClosePromise(fullClose, 'matching');
  expect(afterCallbacks).toBe(scope.selectClosePromise(fullClose, 'matching'));
  expect(afterCallbacks).toBeInstanceOf(Promise);
});

test.each([
  ['native', async () => CloseCallbackScope],
  ['browser', loadBrowserCloseCallbackScope],
])(
  '%s default close identity remains active through a nested async close callback',
  async (_, loadScope) => {
    const Scope = await loadScope();
    const scope = new Scope('build-owner');
    const fullClose = new Promise<void>(() => {});
    let activeAfterSuspension = false;
    let selectedClose!: Promise<void>;

    const wrappedCloseCallback = scope.wrapCallbacks(async () => {
      await scope.runWithCloseIdentity('native-output', async () => {
        await Promise.resolve();
        activeAfterSuspension = scope.hasActiveCallback();
        selectedClose = scope.selectClosePromise(fullClose, 'build-owner');
        await selectedClose;
      });
    });

    await wrappedCloseCallback();

    expect(activeAfterSuspension).toBe(true);
    expect(selectedClose).not.toBe(fullClose);
    await expect(selectedClose).resolves.toBeUndefined();
    expect(scope.hasActiveCallback()).toBe(false);
  },
);

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

test('browser callback-free normalization leaves the close context configurable', async () => {
  const browserModule = await buildBrowserCloseCallbackModule();
  const scope = new browserModule.CloseCallbackScope();
  await expect(normalizePluginOption(undefined, scope)).resolves.toEqual([]);

  let storageCreations = 0;
  browserModule.configureAsyncContext({
    createStorage() {
      storageCreations += 1;
      return new AsyncLocalStorage<unknown>();
    },
  });
  expect(storageCreations).toBe(0);

  const fullClose = new Promise<void>(() => {});
  let callbackClose: Promise<void> | undefined;
  await scope.run(async () => {
    await Promise.resolve();
    callbackClose = scope.selectClosePromise(fullClose);
  });

  expect(storageCreations).toBe(1);
  expect(callbackClose).toBeDefined();
  expect(callbackClose).not.toBe(fullClose);
  await expect(callbackClose).resolves.toBeUndefined();
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
])('%s callback-return then methods preserve deferred invocation timing', async (_, loadScope) => {
  const Scope = await loadScope();
  const scope = new Scope();
  const events: string[] = [];
  const thenable = {
    // oxlint-disable-next-line unicorn/no-thenable -- verifies Promise-like invocation timing
    then(resolve: (value: string) => void) {
      events.push('then');
      resolve('settled');
    },
  };

  const result = scope.run(() => {
    events.push('callback');
    return thenable;
  }) as unknown as Promise<string>;
  events.push('after-run');

  expect(events).toEqual(['callback', 'after-run']);
  await expect(result).resolves.toBe('settled');
  expect(events).toEqual(['callback', 'after-run', 'then']);
});

test.each([
  ['native', async () => CloseCallbackScope],
  ['browser', loadBrowserCloseCallbackScope],
])('%s callback-return then method throws become promise rejections', async (_, loadScope) => {
  const Scope = await loadScope();
  const scope = new Scope();
  const thenError = new Error('then failed');
  const thenable = {
    // oxlint-disable-next-line unicorn/no-thenable -- verifies Promise-like rejection behavior
    then() {
      throw thenError;
    },
  };

  let result: Promise<unknown> | undefined;
  expect(() => {
    result = scope.run(() => thenable) as unknown as Promise<unknown>;
  }).not.toThrow();
  await expect(result).rejects.toBe(thenError);
});

test.each([
  ['native', async () => CloseCallbackScope],
  ['browser', loadBrowserCloseCallbackScope],
])('%s callback-return thenables can request reentrant close', async (_, loadScope) => {
  const Scope = await loadScope();
  const scope = new Scope();
  const fullClose = new Promise<void>(() => {});
  let reentrantClose: Promise<void> | undefined;
  const thenable = {
    // oxlint-disable-next-line unicorn/no-thenable -- verifies callback-result assimilation
    then(resolve: (value: string) => void) {
      reentrantClose = scope.selectClosePromise(fullClose);
      resolve('settled');
    },
  };

  await expect(scope.run(() => thenable) as unknown as Promise<string>).resolves.toBe('settled');
  expect(reentrantClose).toBeDefined();
  expect(reentrantClose).not.toBe(fullClose);
  await expect(reentrantClose).resolves.toBeUndefined();
});

test('browser callback-return thenables do not grant close privilege to later microtasks', async () => {
  const BrowserCloseCallbackScope = await loadBrowserCloseCallbackScope();
  const scope = new BrowserCloseCallbackScope();
  const fullClose = new Promise<void>(() => {});
  let synchronousClose: Promise<void> | undefined;
  let microtaskClose: Promise<void> | undefined;
  let resolveMicrotask!: () => void;
  const microtaskRan = new Promise<void>((resolve) => {
    resolveMicrotask = resolve;
  });
  const thenable = {
    // oxlint-disable-next-line unicorn/no-thenable -- verifies browser privilege lifetime
    then(resolve: (value: string) => void) {
      synchronousClose = scope.selectClosePromise(fullClose);
      queueMicrotask(() => {
        microtaskClose = scope.selectClosePromise(fullClose);
        resolveMicrotask();
      });
      resolve('settled');
    },
  };

  await expect(scope.run(() => thenable) as unknown as Promise<string>).resolves.toBe('settled');
  await microtaskRan;
  expect(synchronousClose).not.toBe(fullClose);
  expect(microtaskClose).toBe(fullClose);
});

// Thenable-assimilation semantics of `trackAsyncCallbackSettlement` (self and
// mutual resolution cycles, mutable self-resolution, nested `then` accessors
// that return undefined / a function / throw) are covered directly in
// async-context.test.ts. `CloseCallbackScope` only supplies the synchronous
// callback runner, which does not take part in that resolution procedure, so
// they are not re-asserted here.

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

test('plugin normalization accepts a thenable that sheds `then` before resolving to itself', async () => {
  let thenCalls = 0;
  const mutableSelf: any = {
    // oxlint-disable-next-line unicorn/no-thenable -- verifies mutable self-resolution
    then(resolve: (value: unknown) => void) {
      thenCalls += 1;
      delete mutableSelf.then;
      resolve(mutableSelf);
    },
  };

  const [plugin] = await normalizePluginOption(mutableSelf);
  expect(plugin).toBe(mutableSelf);
  expect(thenCalls).toBe(1);
});

test('plugin normalization accepts a thenable whose `then` turns non-callable before resolving to itself', async () => {
  let thenCalls = 0;
  const mutableSelf: any = {
    // oxlint-disable-next-line unicorn/no-thenable -- verifies mutable self-resolution
    then(resolve: (value: unknown) => void) {
      thenCalls += 1;
      // oxlint-disable-next-line unicorn/no-thenable -- turns `then` non-callable mid-resolution
      mutableSelf.then = null;
      resolve(mutableSelf);
    },
  };

  const [plugin] = await normalizePluginOption(mutableSelf);
  expect(plugin).toBe(mutableSelf);
  expect(thenCalls).toBe(1);
});

test('plugin normalization accepts a `then` accessor that deletes itself before resolving to itself', async () => {
  let thenReads = 0;
  // oxlint-disable-next-line unicorn/no-thenable -- verifies mutable accessor self-resolution
  const mutableAccessorSelf: any = Object.defineProperty({}, 'then', {
    configurable: true,
    get() {
      thenReads += 1;
      Reflect.deleteProperty(mutableAccessorSelf, 'then');
      return (resolve: (value: unknown) => void) => resolve(mutableAccessorSelf);
    },
  });

  const [plugin] = await normalizePluginOption(mutableAccessorSelf);
  expect(plugin).toBe(mutableAccessorSelf);
  expect(thenReads).toBe(1);
});

test('plugin normalization rejects a thenable that stays thenable while resolving to itself', async () => {
  let thenCalls = 0;
  const persistentSelf: any = {
    // oxlint-disable-next-line unicorn/no-thenable -- verifies cyclic thenable rejection
    then(resolve: (value: unknown) => void) {
      thenCalls += 1;
      resolve(persistentSelf);
    },
  };

  await expect(normalizePluginOption(persistentSelf)).rejects.toThrow(
    new TypeError('Thenable cycle detected while flattening values'),
  );
  expect(thenCalls).toBe(1);
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

test.each([
  ['native', async () => CloseCallbackScope],
  ['browser', loadBrowserCloseCallbackScope],
  ['unscoped', async () => undefined],
])('%s plugin thenables are invoked in a later promise job', async (_, loadScope) => {
  const Scope = await loadScope();
  const scope = Scope ? new Scope() : undefined;
  const events: string[] = [];
  let plugin = { name: 'stale' };
  const thenable = {
    // oxlint-disable-next-line unicorn/no-thenable -- verifies Promise-like invocation timing
    then(resolve: (value: typeof plugin) => void) {
      events.push('then');
      resolve(plugin);
    },
  };

  const normalized = normalizePluginOption(thenable, scope);
  events.push('after-call');
  plugin = { name: 'updated' };

  await expect(normalized).resolves.toEqual([{ name: 'updated' }]);
  expect(events).toEqual(['after-call', 'then']);
});

function createThenableArray(
  elements: unknown[],
  resolveTo: unknown,
  kind: 'data' | 'accessor',
): { array: unknown[]; thenReads: () => number } {
  let thenReads = 0;
  const then = (resolve: (value: unknown) => void) => resolve(resolveTo);
  const array: unknown[] = [...elements];
  if (kind === 'data') {
    // oxlint-disable-next-line unicorn/no-thenable -- an array that is also thenable
    Object.assign(array, { then });
  } else {
    // oxlint-disable-next-line unicorn/no-thenable -- an array with an accessor-backed then
    Object.defineProperty(array, 'then', {
      get() {
        thenReads += 1;
        return then;
      },
    });
  }
  return { array, thenReads: () => thenReads };
}

test.each([
  ['data', 'data', [{ name: 'spread' }]],
  ['accessor', 'accessor', [{ name: 'spread' }]],
  ['empty', 'data', []],
])('plugin arrays that are thenable resolve through `then` (%s)', async (_, kind, elements) => {
  const plugins = [{ name: 'from-then' }];
  const direct = createThenableArray(elements, plugins, kind);
  await expect(normalizePluginOption(direct.array)).resolves.toEqual(plugins);

  const nested = createThenableArray(elements, plugins, kind);
  await expect(normalizePluginOption([[nested.array]])).resolves.toEqual(plugins);

  const resolved = createThenableArray(elements, plugins, kind);
  await expect(
    normalizePluginOption({
      // oxlint-disable-next-line unicorn/no-thenable -- resolves to a thenable array
      then(resolve: (value: unknown) => void) {
        resolve(resolved.array);
      },
    }),
  ).resolves.toEqual(plugins);

  if (kind === 'accessor') {
    expect([direct.thenReads(), nested.thenReads(), resolved.thenReads()]).toEqual([1, 1, 1]);
  }
});

test('plugin thenable results are classified when `resolve` runs, before later mutation', async () => {
  const plugin: { name: string; then?: unknown } = { name: 'resolved' };
  const thenable = {
    // oxlint-disable-next-line unicorn/no-thenable -- verifies resolve-time classification
    then(resolve: (value: typeof plugin) => void) {
      resolve(plugin);
      // The Promise Resolution Procedure has read `then` by the time `resolve`
      // returns; a `then` attached afterwards is not assimilated.
      // oxlint-disable-next-line unicorn/no-thenable -- mutates after resolution
      plugin.then = (innerResolve: (value: unknown) => void) => innerResolve({ name: 'late' });
    },
  };

  const [normalized] = await normalizePluginOption(thenable);
  expect(normalized).toBe(plugin);
});

// A plugin-option accessor runs while its build is already registered as an
// active build, so `bundle.close()` must be acknowledged reentrantly there:
// unless the accessor itself runs inside the close-callback scope, a thenable
// derived from that close waits for a close that waits for this build.
const PLUGIN_ACCESSOR_DEADLINE_MS = 5_000;
const VIRTUAL_ENTRY_ID = '\0close-callback-scope-entry';

function createVirtualEntryPlugin(): Plugin {
  return {
    name: 'close-callback-scope-virtual-entry',
    resolveId(id) {
      if (id === VIRTUAL_ENTRY_ID) return VIRTUAL_ENTRY_ID;
    },
    load(id) {
      if (id === VIRTUAL_ENTRY_ID) return 'export const value = 1;';
    },
  };
}

function settleWithinDeadline<T>(operation: Promise<T>, label: string): Promise<T> {
  let timer: ReturnType<typeof setTimeout>;
  const deadline = new Promise<never>((_, reject) => {
    timer = setTimeout(
      () => reject(new Error(`${label} did not settle within ${PLUGIN_ACCESSOR_DEADLINE_MS}ms`)),
      PLUGIN_ACCESSOR_DEADLINE_MS,
    );
  });
  return Promise.race([operation, deadline]).finally(() => clearTimeout(timer));
}

test(
  'output plugin option accessors closing the bundle do not deadlock generate',
  { timeout: PLUGIN_ACCESSOR_DEADLINE_MS * 3 },
  async ({ onTestFinished }) => {
    const bundle = await rolldown({
      input: VIRTUAL_ENTRY_ID,
      plugins: [createVirtualEntryPlugin()],
    });
    onTestFinished(() => settleWithinDeadline(bundle.close(), 'close()').catch(() => {}));

    let accessorCloses = 0;
    const outputOptions = Object.defineProperty({}, 'plugins', {
      configurable: true,
      enumerable: true,
      get() {
        accessorCloses += 1;
        return bundle.close().then(() => []);
      },
    });

    const output = await settleWithinDeadline(bundle.generate(outputOptions), 'generate()');

    expect(accessorCloses).toBeGreaterThan(0);
    expect(output.output[0].code).toContain('const value = 1');
  },
);

test(
  'output plugin array index accessors closing the bundle do not deadlock generate',
  { timeout: PLUGIN_ACCESSOR_DEADLINE_MS * 3 },
  async ({ onTestFinished }) => {
    const bundle = await rolldown({
      input: VIRTUAL_ENTRY_ID,
      plugins: [createVirtualEntryPlugin()],
    });
    onTestFinished(() => settleWithinDeadline(bundle.close(), 'close()').catch(() => {}));

    let accessorCloses = 0;
    const outputPlugins = Object.defineProperties([], {
      0: {
        configurable: true,
        enumerable: true,
        get() {
          accessorCloses += 1;
          return bundle.close().then(() => ({ name: 'deferred-output-plugin' }));
        },
      },
      length: { value: 1 },
    });

    const output = await settleWithinDeadline(
      bundle.generate({ plugins: outputPlugins }),
      'generate()',
    );

    expect(accessorCloses).toBeGreaterThan(0);
    expect(output.output[0].code).toContain('const value = 1');
  },
);

test(
  'input plugin array index accessors closing the bundle do not deadlock generate',
  { timeout: PLUGIN_ACCESSOR_DEADLINE_MS * 3 },
  async ({ onTestFinished }) => {
    const entryPlugin = createVirtualEntryPlugin();
    let bundle: Awaited<ReturnType<typeof rolldown>> | undefined;
    let accessorCloses = 0;
    const inputPlugins = Object.defineProperties([], {
      0: {
        configurable: true,
        enumerable: true,
        get() {
          // `rolldown()` reads this accessor before the bundle exists.
          if (!bundle) return entryPlugin;
          accessorCloses += 1;
          return bundle.close().then(() => entryPlugin);
        },
      },
      length: { value: 1 },
    });

    bundle = await rolldown({ input: VIRTUAL_ENTRY_ID, plugins: inputPlugins });
    onTestFinished(() => settleWithinDeadline(bundle!.close(), 'close()').catch(() => {}));

    const output = await settleWithinDeadline(bundle.generate({}), 'generate()');

    expect(accessorCloses).toBeGreaterThan(0);
    expect(output.output[0].code).toContain('const value = 1');
  },
);

let browserCloseCallbackScopePromise: Promise<typeof CloseCallbackScope> | undefined;
let browserCloseCallbackModuleIndex = 0;

interface BrowserCloseCallbackModule {
  CloseCallbackScope: typeof CloseCallbackScope;
  configureAsyncContext: typeof configureAsyncContext;
}

function loadBrowserCloseCallbackScope(): Promise<typeof CloseCallbackScope> {
  return (browserCloseCallbackScopePromise ??= buildBrowserCloseCallbackScope());
}

async function buildBrowserCloseCallbackScope(): Promise<typeof CloseCallbackScope> {
  return (await buildBrowserCloseCallbackModule()).CloseCallbackScope;
}

async function buildBrowserCloseCallbackModule(): Promise<BrowserCloseCallbackModule> {
  const entryId = '\0browser-close-callback-scope-entry';
  const scopePath = path.resolve(import.meta.dirname, '../src/utils/close-callback-scope.ts');
  const asyncContextPath = path.resolve(import.meta.dirname, '../src/utils/async-context.ts');
  const prototypeChainPath = path.resolve(import.meta.dirname, '../src/utils/prototype-chain.ts');
  const bundle = await rollup({
    input: entryId,
    plugins: [
      {
        name: 'browser-close-callback-scope',
        resolveId(id, importer) {
          if (id === entryId) return entryId;
          if (importer === entryId && id === 'close-callback-scope') return scopePath;
          if (importer === entryId && id === 'async-context') return asyncContextPath;
          if (id === 'node:async_hooks') return '\0async-hooks';
          if (importer === scopePath && id === './async-context') {
            return asyncContextPath;
          }
          if (importer === asyncContextPath && id === './prototype-chain') {
            return prototypeChainPath;
          }
        },
        load(id) {
          if (id === entryId) {
            return [
              "export { CloseCallbackScope } from 'close-callback-scope';",
              "export { configureAsyncContext } from 'async-context';",
            ].join('\n');
          }
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
    return (await import(
      `data:text/javascript;base64,${Buffer.from(code).toString('base64')}#browser-close-callback-scope-${browserCloseCallbackModuleIndex++}`
    )) as BrowserCloseCallbackModule;
  } finally {
    await bundle.close();
  }
}
