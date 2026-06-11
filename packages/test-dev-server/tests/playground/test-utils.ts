// Shared helpers for the browser e2e specs, exposed via the `~utils` alias
// (configured in vitest.config.e2e.mts). Ported from Vite's
// `playground/test-utils.ts`. A spec does `import { page, editFile, ... } from
// '~utils'`.

import nodeFs from 'node:fs';
import nodePath from 'node:path';
import type { ConsoleMessage } from 'playwright';
import { expect } from 'vitest';
import {
  type DevStatus,
  getBuildSeq as getBuildSeqByUrl,
  getModuleRegistrationSeq as getModuleRegistrationSeqByUrl,
  waitForBuildStable as waitForBuildStableByUrl,
  waitForModuleRegistration as waitForModuleRegistrationByUrl,
  waitForNextBuild as waitForNextBuildByUrl,
} from '../src/dev-status';
import { page, serverUrl, testDir } from './vitest-setup';

export * from './vitest-setup';

// --- File helpers (all relative to the current spec's `testDir`) ------------

export function readFile(filename: string): string {
  return nodeFs.readFileSync(nodePath.resolve(testDir, filename), 'utf-8');
}

/**
 * Edit a file in the current playground's temp copy. The replacer must produce
 * a change (a no-op edit means the test's intent didn't take, so we warn).
 */
export function editFile(filename: string, replacer: (content: string) => string): void {
  const filePath = nodePath.resolve(testDir, filename);
  const content = nodeFs.readFileSync(filePath, 'utf-8');
  const newContent = replacer(content);
  if (content === newContent) {
    console.warn(`[editFile] No changes detected for ${filename}`);
    return;
  }
  nodeFs.writeFileSync(filePath, newContent, 'utf-8');
}

export function addFile(filename: string, content: string): void {
  const filePath = nodePath.resolve(testDir, filename);
  nodeFs.mkdirSync(nodePath.dirname(filePath), { recursive: true });
  nodeFs.writeFileSync(filePath, content, 'utf-8');
}

export function removeFile(filename: string): void {
  nodeFs.unlinkSync(nodePath.resolve(testDir, filename));
}

// --- `/_dev/status` helpers, defaulting to the current spec's server --------

export function waitForNextBuild(currentBuildSeq: number, timeoutMs?: number): Promise<DevStatus> {
  return waitForNextBuildByUrl(serverUrl, currentBuildSeq, timeoutMs);
}

export function waitForBuildStable(stableMs?: number, timeoutMs?: number): Promise<DevStatus> {
  return waitForBuildStableByUrl(serverUrl, stableMs, timeoutMs);
}

export function waitForModuleRegistration(
  currentSeq: number,
  timeoutMs?: number,
): Promise<DevStatus> {
  return waitForModuleRegistrationByUrl(serverUrl, currentSeq, timeoutMs);
}

export function getModuleRegistrationSeq(): Promise<number> {
  return getModuleRegistrationSeqByUrl(serverUrl);
}

export function getBuildSeq(): Promise<number> {
  return getBuildSeqByUrl(serverUrl);
}

export type { DevStatus };

// --- Log-driven synchronization (Vite's `untilBrowserLogAfter`) -------------

interface PromiseWithResolvers<T> {
  promise: Promise<T>;
  resolve: (value: T | PromiseLike<T>) => void;
  reject: (reason?: unknown) => void;
}
function promiseWithResolvers<T>(): PromiseWithResolvers<T> {
  let resolve!: (value: T | PromiseLike<T>) => void;
  let reject!: (reason?: unknown) => void;
  const promise = new Promise<T>((res, rej) => {
    resolve = res;
    reject = rej;
  });
  return { promise, resolve, reject };
}

type UntilBrowserLogAfterCallback = (logs: string[]) => PromiseLike<void> | void;

/**
 * Run `operation` and resolve once the browser console has emitted the target
 * log(s). This is the deterministic alternative to sleeping after an action
 * that produces an async, log-announced effect (HMR patch applied, websocket
 * reconnected, page reloaded). Captures `console.debug` too — Playwright sees
 * it even though DevTools hides it by default.
 */
export async function untilBrowserLogAfter(
  operation: () => unknown,
  target: string | RegExp | Array<string | RegExp>,
  expectOrder = false,
  callback?: UntilBrowserLogAfterCallback,
): Promise<string[]> {
  const promise = untilBrowserLog(target, expectOrder);
  await operation();
  const logs = await promise;
  if (callback) {
    await callback(logs);
  }
  return logs;
}

function untilBrowserLog(
  target: string | RegExp | Array<string | RegExp>,
  expectOrder: boolean,
): Promise<string[]> {
  const { promise, resolve, reject } = promiseWithResolvers<string[]>();
  const logs: string[] = [];

  const isMatch = (matcher: string | RegExp) => (text: string) =>
    typeof matcher === 'string' ? text === matcher : matcher.test(text);

  let processMsg: (text: string) => boolean;
  if (Array.isArray(target)) {
    if (expectOrder) {
      const remaining = [...target];
      processMsg = (text) => {
        const next = remaining.shift();
        if (next !== undefined) {
          expect(text).toMatch(next);
        }
        return remaining.length === 0;
      };
    } else {
      const remaining = target.map(isMatch);
      processMsg = (text) => {
        const idx = remaining.findIndex((m) => m(text));
        if (idx >= 0) {
          remaining.splice(idx, 1);
        }
        return remaining.length === 0;
      };
    }
  } else {
    processMsg = isMatch(target);
  }

  const handleMsg = (msg: ConsoleMessage) => {
    try {
      const text = msg.text();
      logs.push(text);
      if (processMsg(text)) {
        page.off('console', handleMsg);
        clearTimeout(timeoutId);
        resolve(logs);
      }
    } catch (err) {
      page.off('console', handleMsg);
      clearTimeout(timeoutId);
      reject(err);
    }
  };

  const timeoutId = setTimeout(() => {
    page.off('console', handleMsg);
    const waitingFor = Array.isArray(target)
      ? expectOrder
        ? target[0]
        : target.join(', ')
      : target;
    reject(new Error(`Timeout waiting for browser logs. Waiting for: ${waitingFor}`));
  }, 5000);

  page.on('console', handleMsg);
  return promise;
}
