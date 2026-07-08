import { expect, test, vi } from 'vitest';
import { WatchResultCloseRegistry } from '../src/api/watch/watcher';
import { CloseCoordinator } from '../src/runtime-lifecycle';

test('terminal drain closes only live superseded results and replays its outcomes', async () => {
  const registry = new WatchResultCloseRegistry();
  const first = vi.fn(async () => {});
  const second = vi.fn(async () => {});
  const third = vi.fn(async () => {});

  const unregisterFirst = registry.register(0, 'first', first);
  registry.register(0, 'second', second);
  unregisterFirst();
  registry.register(0, 'third', third);

  const firstDrain = registry.drain();
  await expect(firstDrain).resolves.toEqual([{ status: 'fulfilled', value: undefined }]);
  expect(first).not.toHaveBeenCalled();
  expect(second).toHaveBeenCalledOnce();
  expect(third).not.toHaveBeenCalled();

  const replay = registry.drain();
  expect(replay).toBe(firstDrain);
  await expect(replay).resolves.toEqual([{ status: 'fulfilled', value: undefined }]);
  expect(second).toHaveBeenCalledOnce();
});

test('starting a replacement build retires the current result before its successor is emitted', async () => {
  const registry = new WatchResultCloseRegistry();
  const current = vi.fn(async () => {});
  registry.register(3, 'current', current);

  registry.beginTaskBuild(3)(true);

  await expect(registry.drain()).resolves.toEqual([{ status: 'fulfilled', value: undefined }]);
  expect(current).toHaveBeenCalledOnce();
});

test('terminal drain excludes a superseded result closed by the native watcher', async () => {
  const registry = new WatchResultCloseRegistry();
  const current = vi.fn(async () => {});
  registry.register(3, 'native-owned', current);

  registry.beginTaskBuild(3)(true);

  await expect(registry.drain(new Set(['native-owned']))).resolves.toEqual([]);
  expect(current).not.toHaveBeenCalled();
});

test('transport-failure drain closes current and pending results without native ownership', async () => {
  const registry = new WatchResultCloseRegistry();
  const current = vi.fn(async () => {});
  const pending = vi.fn(async () => {});
  registry.register(1, 'current', current);
  registry.register(2, 'pending', pending);
  registry.beginTaskBuild(2);

  await expect(registry.drain(new Set(), true)).resolves.toEqual([
    { status: 'fulfilled', value: undefined },
    { status: 'fulfilled', value: undefined },
  ]);
  expect(current).toHaveBeenCalledOnce();
  expect(pending).toHaveBeenCalledOnce();
});

test('canceling a pending build keeps the current result native-owned', async () => {
  const registry = new WatchResultCloseRegistry();
  const current = vi.fn(async () => {});
  registry.register(3, 'current', current);

  const finishTaskBuildStart = registry.beginTaskBuild(3);
  registry.cancelPendingBuilds();
  finishTaskBuildStart(true);

  await expect(registry.drain()).resolves.toEqual([]);
  expect(current).not.toHaveBeenCalled();
});

test('closing a result during BUNDLE_START removes its pending build handoff', async () => {
  const registry = new WatchResultCloseRegistry();
  const current = vi.fn(async () => {});
  const unregister = registry.register(3, 'current', current);

  const finishTaskBuildStart = registry.beginTaskBuild(3);
  await current();
  unregister();
  finishTaskBuildStart(true);

  await expect(registry.drain()).resolves.toEqual([]);
  expect(current).toHaveBeenCalledOnce();
});

test('unregistering a superseded result during drain does not duplicate or cancel its close', async () => {
  const registry = new WatchResultCloseRegistry();
  let releaseClose!: () => void;
  const closeGate = new Promise<void>((resolve) => {
    releaseClose = resolve;
  });
  const close = vi.fn(() => closeGate);
  const unregister = registry.register(0, 'first', close);
  registry.register(
    0,
    'second',
    vi.fn(async () => {}),
  );

  const firstDrain = registry.drain();
  await Promise.resolve();
  expect(close).toHaveBeenCalledOnce();

  unregister();
  const concurrentDrain = registry.drain();
  expect(concurrentDrain).toBe(firstDrain);
  expect(close).toHaveBeenCalledOnce();

  releaseClose();
  await expect(firstDrain).resolves.toEqual([{ status: 'fulfilled', value: undefined }]);
  expect(close).toHaveBeenCalledOnce();
});

test('distinct result closes preserve duplicate references to one shared error object', async () => {
  const registry = new WatchResultCloseRegistry();
  const sharedError = new Error('shared close failure');
  const first = vi.fn(async () => {
    throw sharedError;
  });
  const second = vi.fn(async () => {
    throw sharedError;
  });
  registry.register(0, 'first', first);
  registry.register(0, 'second', second);
  registry.register(
    0,
    'third',
    vi.fn(async () => {}),
  );

  const outcomes = await registry.drain();
  expect(outcomes).toEqual([
    { status: 'rejected', reason: sharedError },
    { status: 'rejected', reason: sharedError },
  ]);
  expect(first).toHaveBeenCalledOnce();
  expect(second).toHaveBeenCalledOnce();
});

test('terminal drain attempts every result close after a synchronous throw', async () => {
  const registry = new WatchResultCloseRegistry();
  const syncError = new Error('synchronous close failure');
  const asyncError = new Error('asynchronous close failure');
  const first = vi.fn(() => {
    throw syncError;
  });
  const second = vi.fn(async () => {
    throw asyncError;
  });
  const third = vi.fn(async () => {});

  registry.register(0, 'first', first);
  registry.register(0, 'second', second);
  registry.register(0, 'third', third);
  registry.register(
    0,
    'fourth',
    vi.fn(async () => {}),
  );

  await expect(registry.drain()).resolves.toEqual([
    { status: 'rejected', reason: syncError },
    { status: 'rejected', reason: asyncError },
    { status: 'fulfilled', value: undefined },
  ]);
  expect(first).toHaveBeenCalledOnce();
  expect(second).toHaveBeenCalledOnce();
  expect(third).toHaveBeenCalledOnce();
});

test('retryable cleanup replays drained result failures without rerunning their closes', async () => {
  const registry = new WatchResultCloseRegistry();
  const resultError = new Error('superseded result failed');
  const workerError = new Error('worker termination failed');
  const resultClose = vi.fn(async () => {
    throw resultError;
  });
  registry.register(0, 'result', resultClose);
  registry.register(
    0,
    'current',
    vi.fn(async () => {}),
  );

  let workerAttempts = 0;
  const attempt = vi.fn(async () => {
    const outcomes = await registry.drain();
    const errors = outcomes.flatMap((outcome) =>
      outcome.status === 'rejected' ? [outcome.reason] : [],
    );
    workerAttempts += 1;
    const retryable = workerAttempts === 1;
    if (retryable) errors.push(workerError);
    return { errors, retryable };
  });
  const coordinator = new CloseCoordinator('watcher close failed');

  const firstClose = coordinator.close(attempt);
  await expect(firstClose).rejects.toMatchObject({
    errors: [resultError, workerError],
  });

  const retryClose = coordinator.close(attempt);
  await expect(retryClose).rejects.toBe(resultError);
  expect(resultClose).toHaveBeenCalledOnce();
  expect(attempt).toHaveBeenCalledTimes(2);

  const replay = coordinator.close(attempt);
  expect(replay).toBe(retryClose);
  await expect(replay).rejects.toBe(resultError);
  expect(resultClose).toHaveBeenCalledOnce();
});
