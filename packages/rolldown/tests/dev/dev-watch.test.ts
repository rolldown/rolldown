import { getDevWatchOptionsForCi } from '@rolldown/test-dev-server';
import crypto from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';
import type { InputOptions, OutputOptions } from 'rolldown';
import type { DevEngine, DevOptions } from 'rolldown/experimental';
import { dev as _dev } from 'rolldown/experimental';
import { sleep } from 'rolldown-tests/utils';
import { test, vi } from 'vitest';

const TEST_RETRY = 3;
const TEST_TIMEOUT = 60_000;

// Wrap dev() to inject usePolling for CI stability.
// PollWatcher uses whole-second mtime comparison, so file edits
// must use editFile() to ensure mtime crosses a second boundary.
function dev(
  inputOptions: InputOptions,
  outputOptions: OutputOptions,
  devOptions: DevOptions,
): Promise<DevEngine> {
  return _dev(inputOptions, outputOptions, {
    ...devOptions,
    watch: {
      ...getDevWatchOptionsForCi(),
      ...devOptions.watch,
    },
  });
}

// Write a file with a 1s sleep beforehand to ensure the PollWatcher's
// whole-second mtime comparison detects the change.
async function editFile(filePath: string, content: string) {
  await sleep(1000);
  fs.writeFileSync(filePath, content);
}

test.concurrent(
  'dev watch exclude',
  { retry: TEST_RETRY, timeout: TEST_TIMEOUT },
  async ({ task, expect, onTestFinished }) => {
    const retryCount = task.result?.retryCount ?? 0;
    const { input, outputDir, dir } = createTestInputAndOutput('dev-include-exclude', retryCount);

    const onOutput = vi.fn();
    const onHmrUpdates = vi.fn();
    const engine = await dev(
      {
        input,
        experimental: { devMode: true },
      },
      { dir: outputDir },
      {
        onOutput,
        onHmrUpdates,
        watch: {
          exclude: '**/main.js',
        },
      },
    );
    onTestFinished(async () => {
      await engine.close();
      if (!process.env.CI) {
        fs.rmSync(dir, { recursive: true, force: true });
      }
    });

    await engine.run();
    await expect.poll(() => onOutput).toHaveBeenCalled();

    // edit file
    onOutput.mockClear();
    onHmrUpdates.mockClear();
    await editFile(input, 'console.log(2)');
    // The input is excluded, so no rebuild or HMR update should fire.
    await sleep(1000);
    expect(onOutput).not.toHaveBeenCalled();
    expect(onHmrUpdates).not.toHaveBeenCalled();
  },
);

test.concurrent(
  'dev watch exclude sanity',
  { retry: TEST_RETRY, timeout: TEST_TIMEOUT },
  async ({ task, expect, onTestFinished }) => {
    const retryCount = task.result?.retryCount ?? 0;
    const { input, outputDir, dir } = createTestInputAndOutput(
      'dev-include-exclude-sanity',
      retryCount,
    );

    const onOutput = vi.fn();
    const onHmrUpdates = vi.fn();
    const engine = await dev(
      {
        input,
        experimental: { devMode: true },
      },
      { dir: outputDir },
      {
        onOutput,
        onHmrUpdates,
        watch: {
          exclude: '**/unrelated.js',
        },
      },
    );
    onTestFinished(async () => {
      await engine.close();
      if (!process.env.CI) {
        fs.rmSync(dir, { recursive: true, force: true });
      }
    });

    await engine.run();
    await expect.poll(() => onOutput).toHaveBeenCalled();

    // edit file
    onHmrUpdates.mockClear();
    await editFile(input, 'console.log(2)');
    // The input is not excluded, so an HMR update should fire.
    await expect.poll(() => onHmrUpdates).toHaveBeenCalled();
  },
);

test.concurrent(
  'dev watch include',
  { retry: TEST_RETRY, timeout: TEST_TIMEOUT },
  async ({ task, expect, onTestFinished }) => {
    const retryCount = task.result?.retryCount ?? 0;
    const { dir: cwd } = createTestWithMultiFiles('dev-include', retryCount, {
      'main.js': `import './dep.js'\nconsole.log(1)`,
      'dep.js': `export const dep = 1`,
    });

    const onOutput = vi.fn();
    const onHmrUpdates = vi.fn();
    const engine = await dev(
      {
        cwd,
        input: 'main.js',
        experimental: { devMode: true },
      },
      { dir: path.join(cwd, 'dist') },
      {
        onOutput,
        onHmrUpdates,
        watch: {
          // Only main.js is watched; dep.js falls outside the allowlist.
          include: '**/main.js',
        },
      },
    );
    onTestFinished(async () => {
      await engine.close();
      if (!process.env.CI) {
        fs.rmSync(cwd, { recursive: true, force: true });
      }
    });

    await engine.run();
    await expect.poll(() => onOutput).toHaveBeenCalled();

    // edit file not matching include
    onHmrUpdates.mockClear();
    await editFile(path.join(cwd, 'dep.js'), `export const dep = 2`);
    // dep.js is outside the include allowlist, so no HMR update should fire.
    await sleep(1000);
    expect(onHmrUpdates).not.toHaveBeenCalled();

    // edit file matching include
    await editFile(path.join(cwd, 'main.js'), `import './dep.js'\nconsole.log(2)`);
    // main.js matches include, so an HMR update should fire.
    await expect.poll(() => onHmrUpdates).toHaveBeenCalled();
  },
);

function createTestInputAndOutput(testLabel: string, retryCount: number) {
  const uniqueId = crypto.randomUUID().slice(0, 8);
  const dirname = `${testLabel}-${uniqueId}-retry${retryCount}`;
  const dir = path.join(import.meta.dirname, 'temp', dirname);
  fs.mkdirSync(dir, { recursive: true });
  const input = path.join(dir, 'main.js');
  fs.writeFileSync(input, 'console.log(1)');
  const outputDir = path.join(dir, 'dist');
  return { input, outputDir, dir };
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
