import assert from 'node:assert/strict';
import { createRequire } from 'node:module';
import { mkdtempSync, readdirSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

import { installCurrentThreadTaskHost } from '../install-current-thread-task-host.mjs';

const require = createRequire(import.meta.url);
const bindingDir = fileURLToPath(new URL('../../../dist/', import.meta.url));
const bindingFiles = readdirSync(bindingDir).filter(
  (name) => name.startsWith('rolldown-binding.') && name.endsWith('.node'),
);
assert.equal(bindingFiles.length, 1);
const binding = require(path.join(bindingDir, bindingFiles[0]));
const stopRuntime = binding.__rolldownTestStopAsyncRuntime;
const startRuntime = binding.__rolldownTestStartAsyncRuntime;
assert.equal(typeof stopRuntime, 'function');
assert.equal(typeof startRuntime, 'function');
const uninstallCurrentThreadTaskHost = installCurrentThreadTaskHost(binding);

const root = mkdtempSync(path.join(tmpdir(), 'rolldown-submission-failure-'));
writeFileSync(path.join(root, 'main.js'), 'export const value = 1;\n');

const bundler = new binding.BindingBundler();
let watcher;
const options = {
  inputOptions: {
    cwd: root,
    input: [{ import: './main.js' }],
    logLevel: binding.BindingLogLevel.Silent,
    onLog() {},
    plugins: [],
  },
  outputOptions: {
    dir: path.join(root, 'dist'),
    plugins: [],
  },
};

try {
  stopRuntime();
  let submissionError;
  try {
    await bundler.generate(options);
  } catch (error) {
    submissionError = error;
  }
  assert.ok(submissionError instanceof Error);
  assert.match(submissionError.message, /the async runtime is stopped/);

  startRuntime();
  const retried = await bundler.generate(options);
  assert.equal(retried?.isBindingErrors, undefined);
  await bundler.close();

  let resolveWatcherEnd;
  const watcherEnd = new Promise((resolve) => {
    resolveWatcherEnd = resolve;
  });
  let watcherBuildStarts = 0;
  let watcherBuildEnds = 0;
  watcher = new binding.BindingWatcher(
    [
      {
        inputOptions: {
          ...options.inputOptions,
          plugins: [],
        },
        outputOptions: {
          dir: path.join(root, 'watch-dist'),
          plugins: [],
        },
      },
    ],
    (event) => {
      if (event.eventKind() !== 'event') return;
      const kind = event.bundleEventKind();
      if (kind === 'BUNDLE_START') {
        watcherBuildStarts += 1;
      } else if (kind === 'BUNDLE_END') {
        watcherBuildEnds += 1;
      } else if (kind === 'END') {
        resolveWatcherEnd();
      }
    },
  );

  stopRuntime();
  let firstRun;
  assert.doesNotThrow(() => {
    firstRun = watcher.run();
  });
  assert.equal(typeof firstRun?.then, 'function');
  await assert.rejects(firstRun, /Watcher coordinator task submission failed:/);
  assert.equal(watcherBuildStarts, 0);
  assert.equal(watcherBuildEnds, 0);

  startRuntime();
  await watcher.run();
  await watcher.run();
  let watcherEndTimeout;
  try {
    await Promise.race([
      watcherEnd,
      new Promise((_, reject) => {
        watcherEndTimeout = setTimeout(
          () => reject(new Error('retried watcher did not finish its initial build')),
          10_000,
        );
      }),
    ]);
  } finally {
    clearTimeout(watcherEndTimeout);
  }
  assert.equal(watcherBuildStarts, 1);
  assert.equal(watcherBuildEnds, 1);
  await watcher.close();

  console.log(
    JSON.stringify({
      generateRetried: true,
      submissionRejected: true,
      watcherBuildEnds,
      watcherBuildStarts,
      watcherRunRejected: true,
    }),
  );
} finally {
  try {
    startRuntime();
  } catch {}
  await watcher?.close().catch(() => {});
  await bundler.close().catch(() => {});
  uninstallCurrentThreadTaskHost();
  rmSync(root, { force: true, recursive: true });
}
