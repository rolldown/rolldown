import { describe, expect, test, vi } from 'vitest';

// @ts-ignore These focused unit tests intentionally reach package source outside the test rootDir.
import { CloseCoordinator, getCloseTerminalErrors } from '../src/runtime-lifecycle';

describe('CloseCoordinator', () => {
  test('publishes the close promise before a synchronous attempt reenters close', async () => {
    const coordinator = new CloseCoordinator('close failed');
    let reentered = false;
    let reentrantClose: Promise<void> | undefined;
    const attempt = vi.fn(async () => {
      if (!reentered) {
        reentered = true;
        reentrantClose = coordinator.close(attempt);
      }
      return { errors: [], retryable: false };
    });

    const first = coordinator.close(attempt);

    await expect(first).resolves.toBeUndefined();
    expect(reentrantClose).toBe(first);
    expect(attempt).toHaveBeenCalledOnce();
  });

  test('coalesces an attempt and retries after a retryable cleanup failure', async () => {
    const cleanupError = new Error('cleanup failed');
    const attempt = vi
      .fn<() => Promise<{ errors: unknown[]; retryable: boolean }>>()
      .mockResolvedValueOnce({ errors: [cleanupError], retryable: true })
      .mockResolvedValue({ errors: [], retryable: false });
    const coordinator = new CloseCoordinator('close failed');

    const first = coordinator.close(attempt);
    const concurrent = coordinator.close(attempt);
    expect(concurrent).toBe(first);
    await expect(first).rejects.toBe(cleanupError);
    expect(attempt).toHaveBeenCalledOnce();

    await expect(coordinator.close(attempt)).resolves.toBeUndefined();
    expect(attempt).toHaveBeenCalledTimes(2);
  });

  test('replays terminal failures without rerunning completed phases', async () => {
    const terminalError = new Error('native close failed');
    const attempt = vi.fn(async () => ({
      errors: [terminalError],
      retryable: false,
    }));
    const coordinator = new CloseCoordinator('close failed');

    const first = coordinator.close(attempt);
    await expect(first).rejects.toBe(terminalError);
    const replay = coordinator.close(attempt);
    expect(replay).toBe(first);
    await expect(replay).rejects.toBe(terminalError);
    expect(attempt).toHaveBeenCalledOnce();
  });

  test('owned cleanup retry projects out terminal diagnostics and preserves their replay', async () => {
    const terminalError = new Error('native close failed');
    const cleanupError = new Error('runtime release failed');
    const attempt = vi
      .fn<() => Promise<{ errors: unknown[]; retryable: boolean; terminalErrors: unknown[] }>>()
      .mockResolvedValueOnce({
        errors: [terminalError, cleanupError],
        retryable: true,
        terminalErrors: [terminalError],
      })
      .mockResolvedValue({
        errors: [terminalError],
        retryable: false,
        terminalErrors: [terminalError],
      });
    const coordinator = new CloseCoordinator('close failed');

    await expect(coordinator.close(attempt)).rejects.toMatchObject({
      errors: [terminalError, cleanupError],
    });
    await expect(coordinator.retryOwnedCleanup(attempt)).resolves.toEqual([terminalError]);

    await expect(coordinator.close(attempt)).rejects.toBe(terminalError);
    expect(attempt).toHaveBeenCalledTimes(2);
  });

  test('owned cleanup projection preserves a same-object cleanup failure', async () => {
    const sharedError = new Error('shared terminal and cleanup failure');
    const attempt = vi
      .fn<() => Promise<{ errors: unknown[]; retryable: boolean; terminalErrors: unknown[] }>>()
      .mockResolvedValue({
        errors: [sharedError, sharedError],
        retryable: true,
        terminalErrors: [sharedError],
      });
    const coordinator = new CloseCoordinator('close failed');

    await expect(coordinator.close(attempt)).rejects.toMatchObject({
      errors: [sharedError, sharedError],
    });
    await expect(coordinator.retryOwnedCleanup(attempt)).rejects.toBe(sharedError);
    expect(attempt).toHaveBeenCalledTimes(2);
  });

  test('owned cleanup failure retains terminal diagnostics for its caller', async () => {
    const terminalError = new Error('native close failed');
    const cleanupError = new Error('runtime release still failed');
    const attempt = vi
      .fn<() => Promise<{ errors: unknown[]; retryable: boolean; terminalErrors: unknown[] }>>()
      .mockResolvedValue({
        errors: [terminalError, cleanupError],
        retryable: true,
        terminalErrors: [terminalError],
      });
    const coordinator = new CloseCoordinator('close failed');

    await expect(coordinator.close(attempt)).rejects.toMatchObject({
      errors: [terminalError, cleanupError],
    });
    const retryError = await coordinator
      .retryOwnedCleanup(attempt)
      .catch((error: unknown) => error);

    expect(retryError).toBe(cleanupError);
    expect(getCloseTerminalErrors(retryError)).toEqual([terminalError]);
    expect(attempt).toHaveBeenCalledTimes(2);
  });
});
