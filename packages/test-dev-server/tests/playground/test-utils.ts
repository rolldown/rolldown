// Shared helpers for the browser e2e specs, imported via the `~utils` alias
// (set up in vitest.config.e2e.mts). Ported from Vite's
// `playground/test-utils.ts`.

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
 * Edit a file in the playground's temp copy. Warns if the replacer changed
 * nothing — that usually means the test's edit missed its target.
 */
export function editFile(filename: string, replacer: (content: string) => string): void {
  const filePath = nodePath.resolve(testDir, filename);
  const content = nodeFs.readFileSync(filePath, 'utf-8');
  const newContent = replacer(content);
  if (content === newContent) {
    // DEBUG (CI flake #9727): a silent no-op means the anchor string was
    // already gone — i.e. the fixture was off-baseline before this edit. Print
    // the current content so the CI log shows what state it was actually in.
    const shown =
      content.length > 200 ? `${content.slice(0, 200)}…(+${content.length - 200})` : content;
    console.warn(
      `[editFile] NOOP ${filename} (replacer changed nothing) current=${JSON.stringify(shown)}`,
    );
    return;
  }
  const clip = (s: string) => (s.length > 120 ? `${s.slice(0, 120)}…(+${s.length - 120})` : s);
  console.log(
    `[editFile] APPLIED ${filename}: ${JSON.stringify(clip(content))} -> ${JSON.stringify(clip(newContent))}`,
  );
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
 * Run `operation`, then wait until the browser console prints the target
 * log(s). Use this instead of sleeping after an action whose effect is
 * announced by a log (HMR applied, websocket reconnected, page reloaded).
 * Also sees `console.debug`, which DevTools hides by default.
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
