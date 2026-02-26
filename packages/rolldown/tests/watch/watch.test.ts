import fs from 'node:fs';
import path from 'node:path';
import type { RolldownWatcher, WatchOptions } from 'rolldown';
import { rolldown, watch as _watch } from 'rolldown';
import { sleep } from 'rolldown-tests/utils';
import { expect, onTestFinished, test, vi } from 'vitest';

// Wrap watch() to inject usePolling for CI stability.
// PollWatcher uses whole-second mtime comparison, so file edits
// must use editFile() to ensure mtime crosses a second boundary.
function watch(input: WatchOptions | WatchOptions[]) {
  const options = Array.isArray(input) ? input : [input];
  for (const opt of options) {
    const existing = opt.watch && typeof opt.watch === 'object' ? opt.watch : {};
    opt.watch = {
      ...existing,
      watcher: { usePolling: true, pollInterval: 50, ...existing.watcher },
    };
  }
  return _watch(Array.isArray(input) ? options : options[0]);
}

// Write a file with a 1s sleep beforehand to ensure the PollWatcher's
// whole-second mtime comparison detects the change.
async function editFile(filePath: string, content: string) {
  await sleep(1000);
  fs.writeFileSync(filePath, content);
}

// Delete a file with a 1s sleep beforehand (same mtime-boundary reason).
async function deleteFile(filePath: string) {
  await sleep(1000);
  fs.unlinkSync(filePath);
}

test.sequential('watch', async () => {
  const { input, output, dir } = await createTestInputAndOutput('watch');
  const foo = path.join(dir, 'foo.js');
  fs.writeFileSync(foo, 'export const foo = 1');
  fs.writeFileSync(input, `import './foo.js'; console.log(1)`);
  await sleep(60);

  const watchChangeUpdateFn = vi.fn();
  const watchChangeCreateFn = vi.fn();
  const watchChangeDeleteFn = vi.fn();
  const closeWatcherFn = vi.fn();
  const watcher = watch({
    input,
    output: { file: output },
    plugins: [
      {
        name: 'test watchChange',
        watchChange(id, event) {
          // The macos emit create event when the file is changed, not sure the reason,
          // so here only check the update event
          if (event.event === 'update') {
            watchChangeUpdateFn();
            expect(id).toBe(input);
          }
          if (event.event === 'create') {
            watchChangeCreateFn();
            expect(id).toBe(foo);
          }
          if (event.event === 'delete') {
            watchChangeDeleteFn();
            expect(id).toBe(foo);
          }
        },
      },
      {
        name: 'test closeWatcher',
        closeWatcher() {
          closeWatcherFn();
        },
      },
    ],
  });

  let errored = false;
  try {
    // should run build once
    await waitBuildFinished(watcher);

    // Test update event
    await editFile(input, `import './foo.js'; console.log(2)`);
    await expect.poll(() => fs.readFileSync(output, 'utf-8')).toContain('console.log(2)');
    // The different platform maybe emit multiple events
    expect(watchChangeUpdateFn).toBeCalled();

    // Test delete event
    await deleteFile(foo);
    await expect.poll(() => watchChangeDeleteFn).toBeCalled();

    // Test create event
    await editFile(foo, 'export const foo = 2');
    await expect.poll(() => watchChangeCreateFn).toBeCalled();
  } catch (e) {
    errored = true;
    throw e;
  } finally {
    await watcher.close();
    if (!errored) {
      expect(closeWatcherFn).toBeCalledTimes(1);
    }
  }
});

test.sequential('watch files after scan stage', async () => {
  const { input, output } = await createTestInputAndOutput('watch-files-after-scan');
  // Ensure file mtime is in a previous second so PollWatcher detects the renderStart write
  await sleep(1000);
  const watcher = watch({
    input,
    output: { file: output },
    plugins: [
      {
        name: 'test',
        renderStart() {
          fs.writeFileSync(input, 'console.log(2)');
        },
      },
    ],
  });
  onTestFinished(() => watcher.close());
  // should run build once
  await waitBuildFinished(watcher);

  await expect.poll(() => fs.readFileSync(output, 'utf-8')).toContain('console.log(2)');
});

test.sequential('watch close', async () => {
  const { input, output } = await createTestInputAndOutput('watch-close');
  const watcher = watch({
    input,
    output: { file: output },
  });
  await waitBuildFinished(watcher);

  await watcher.close();
  // edit file
  fs.writeFileSync(input, 'console.log(3)');
  // The watcher is closed, so the output file should not be updated
  await expect.poll(() => fs.readFileSync(output, 'utf-8')).toContain('console.log(1)');
});

test.sequential('watch event', async () => {
  const { input, outputDir } = await createTestInputAndOutput('watch-event');
  const watcher = watch({
    input,
    output: { dir: outputDir },
    watch: {
      buildDelay: 50,
    },
  });

  const closeFn = vi.fn();
  let errored = false;
  try {
    const events: any[] = [];
    watcher.on('event', (event) => {
      if (event.code === 'BUNDLE_END') {
        expect(event.output).toEqual([outputDir]);
        expect(event.duration).toBeTypeOf('number');
        events.push({ code: 'BUNDLE_END' });
      } else {
        events.push(event);
      }
    });
    const restartFn = vi.fn();
    watcher.on('restart', restartFn);
    watcher.on('close', closeFn);
    const changeFn = vi.fn();
    watcher.on('change', (id, event) => {
      // The macos emit create event when the file is changed, not sure the reason,
      // so here only check the update event
      if (event.event === 'update') {
        changeFn();
        expect(id).toBe(input);
      }
    });

    // test first build event
    await expect
      .poll(() => events)
      .toEqual([
        { code: 'START' },
        { code: 'BUNDLE_START' },
        { code: 'BUNDLE_END' },
        { code: 'END' },
      ]);

    // edit file
    events.length = 0;
    await editFile(input, 'console.log(3)');
    // Note: The different platform maybe emit multiple events
    await expect
      .poll(() => events)
      .toEqual([
        { code: 'START' },
        { code: 'BUNDLE_START' },
        { code: 'BUNDLE_END' },
        { code: 'END' },
      ]);
    expect(restartFn).toBeCalled();
    expect(changeFn).toBeCalled();
  } catch (e) {
    errored = true;
    throw e;
  } finally {
    await watcher.close();
    if (!errored) {
      // the listener is called with async
      await expect.poll(() => closeFn).toBeCalled();
    }
  }
});

test.sequential('watch event off', async () => {
  const { input, outputDir } = await createTestInputAndOutput('watch-event-off');
  const watcher = watch({
    input,
    output: { dir: outputDir },
    watch: {
      buildDelay: 50,
    },
  });
  const eventFn = vi.fn();
  watcher.on('event', eventFn);
  onTestFinished(() => watcher.close());
  await waitBuildFinished(watcher);
  expect(eventFn).toHaveBeenCalled();

  eventFn.mockClear();
  watcher.off('event', eventFn);

  await editFile(input, 'console.log(12)');
  await waitBuildFinished(watcher);
  expect(eventFn).not.toHaveBeenCalled();
});

test.sequential('watch BUNDLE_END event result.close() + closeBundle', async () => {
  const { input, outputDir } = await createTestInputAndOutput('watch-event-close-closeBundle');
  const closeBundleFn = vi.fn();
  const watcher = watch({
    input,
    output: { dir: outputDir },
    plugins: [
      {
        name: 'test',
        closeBundle: closeBundleFn,
      },
    ],
  });
  watcher.on('event', async (event) => {
    if (event.code === 'BUNDLE_END') {
      await event.result.close();
    }
  });
  onTestFinished(() => watcher.close());
  await waitBuildFinished(watcher);

  expect(closeBundleFn).toBeCalledTimes(1);

  // The `result.close` could be call multiply times.
  await editFile(input, 'console.log(3)');
  await waitBuildFinished(watcher);
  expect(closeBundleFn).toBeCalledTimes(2);
});

test.sequential('watch ERROR event result.close() + closeBundle', async () => {
  const { input, outputDir } = await createTestInputAndOutput(
    'watch-event-ERROR-close-closeBundle',
  );
  const closeBundleFn = vi.fn();
  const watcher = watch({
    input,
    output: { dir: outputDir },
    plugins: [
      {
        name: 'test',
        buildStart() {
          throw new Error('test error');
        },
        closeBundle: closeBundleFn,
      },
    ],
  });
  watcher.on('event', async (event) => {
    if (event.code === 'ERROR') {
      await event.result.close();
    }
  });
  onTestFinished(() => watcher.close());

  // build error call once + result.close() call once
  await expect.poll(() => closeBundleFn).toBeCalledTimes(2);
});

test.sequential('watch BUNDLE_END event output + "file" option', async () => {
  const { input, output } = await createTestInputAndOutput('watch-event');
  const watcher = watch({
    input,
    output: { file: output },
  });
  onTestFinished(() => watcher.close());

  const eventFn = vi.fn();
  watcher.on('event', (event) => {
    if (event.code === 'BUNDLE_END') {
      eventFn();
      expect(event.output).toEqual([output]);
    }
  });

  // test first build event
  await expect.poll(() => eventFn).toBeCalled();
});

test.sequential('watch event avoid deadlock #2806', async () => {
  const { input, output } = await createTestInputAndOutput('watch-event-avoid-dead-lock');
  const watcher = watch({
    input,
    output: { file: output },
  });
  onTestFinished(() => watcher.close());

  const testFn = vi.fn();
  let listening = false;
  watcher.on('event', (event) => {
    if (event.code === 'BUNDLE_END' && !listening) {
      listening = true;
      // shouldn't deadlock
      watcher.on('event', () => {
        if (event.code === 'BUNDLE_END') {
          testFn();
        }
      });
    }
  });

  await waitBuildFinished(watcher);

  await editFile(input, 'console.log(2)');
  await expect.poll(() => testFn).toBeCalled();
});

test.sequential('watch skipWrite', async () => {
  const { input, output } = await createTestInputAndOutput('watch-skipWrite');
  const watcher = watch({
    input,
    output: { file: output },
    watch: {
      skipWrite: true,
    },
  });
  onTestFinished(() => watcher.close());
  await waitBuildFinished(watcher);

  expect(fs.existsSync(output)).toBe(false);
});

test.sequential('#5260', async () => {
  createTestWithMultiFiles('issue-5260', {
    'main.js': `import './foo.js'`,
    'foo.js': `console.log('foo')`,
  });
  const cwd = path.join(import.meta.dirname, 'temp', 'issue-5260');
  const watcher = watch({
    cwd,
    input: 'main.js',
    watch: {
      buildDelay: 50,
    },
    experimental: {
      incrementalBuild: true,
    },
  });
  onTestFinished(() => watcher.close());
  await waitBuildFinished(watcher);

  watcher.clear('event');

  await editFile(path.join(cwd, 'main.js'), `import('./foo.js')`);

  await waitBuildFinished(watcher);
});

test.sequential('incremental-watch-modify-entry-module', async () => {
  createTestWithMultiFiles('incremental-watch-modify-entry-module', {
    'main.js': `
import {a} from './foo.js'
console.log(a)
`,
    'foo.js': `export const a = 10000`,
  });
  const cwd = path.join(import.meta.dirname, 'temp', 'incremental-watch-modify-entry-module');
  const watcher = watch({
    cwd,
    input: 'main.js',
    watch: {
      buildDelay: 50,
    },
    experimental: {
      incrementalBuild: true,
    },
  });
  onTestFinished(() => watcher.close());
  await waitBuildFinished(watcher);

  watcher.clear('event');
  expect(fs.readdirSync(path.join(cwd, 'dist'))).toHaveLength(1);

  await editFile(
    path.join(cwd, 'main.js'),
    `
import {a} from './foo.js'
console.log(a + 1000)
`,
  );

  await waitBuildFinished(watcher);
  expect(fs.readdirSync(path.join(cwd, 'dist'))).toHaveLength(1);
});

test.sequential('watch sync ast of newly added ast', async () => {
  createTestWithMultiFiles('sync-ast-of-newly-added-modules', {
    'main.js': `import ('./d1.js').then(console.log)`,
    'd1.js': `export const a = 1`,
    'd2.js': `export const b = 2`,
  });
  const cwd = path.join(import.meta.dirname, 'temp', 'sync-ast-of-newly-added-modules');
  const watcher = watch({
    cwd,
    input: 'main.js',
    watch: {
      buildDelay: 50,
    },
    experimental: {
      incrementalBuild: true,
    },
  });
  onTestFinished(() => watcher.close());
  await waitBuildFinished(watcher);

  watcher.clear('event');

  await editFile(
    path.join(cwd, 'main.js'),
    `import ('./d1.js').then(console.log);import ('./d2.js').then(console.log)`,
  );

  await waitBuildFinished(watcher);
});

test.sequential('watch buildDelay', async () => {
  const { input, output } = await createTestInputAndOutput('watch-buildDelay');
  const watcher = watch({
    input,
    output: { file: output },
    watch: {
      buildDelay: 50,
    },
  });
  onTestFinished(() => watcher.close());
  await waitBuildFinished(watcher);

  const restartFn = vi.fn();
  watcher.on('restart', restartFn);

  // Sleep to ensure mtime crosses second boundary from initial creation
  await sleep(1000);
  fs.writeFileSync(input, 'console.log(4)');
  await sleep(20);
  fs.writeFileSync(input, 'console.log(5)');

  // sleep 200ms to wait the build finished, if the buildDelay is working, the restartFn should be called once
  await sleep(200);
  await expect.poll(() => fs.readFileSync(output, 'utf-8')).toContain('console.log(5)');
  expect(restartFn).toBeCalledTimes(1);
});

test.sequential('PluginContext addWatchFile', async () => {
  const { input, output } = await createTestInputAndOutput('addWatchFile');
  const { input: foo } = await createTestInputAndOutput('addWatchFile-foo');
  const watcher = watch({
    input,
    output: { file: output },
    plugins: [
      {
        name: 'test',
        buildStart() {
          this.addWatchFile(foo);
        },
      },
    ],
  });
  onTestFinished(() => watcher.close());

  await waitBuildFinished(watcher);

  const changeFn = vi.fn();
  watcher.on('change', (id, event) => {
    // The macos emit create event when the file is changed, not sure the reason,
    // so here only check the update event
    if (event.event === 'update') {
      changeFn();
      expect(id).toBe(foo);
    }
  });

  // edit file
  await editFile(foo, 'console.log(2)\n');
  await expect.poll(() => changeFn).toBeCalled();
});

test.sequential('watch include/exclude', async () => {
  const { input, output } = await createTestInputAndOutput('include-exclude');
  const watcher = watch({
    input,
    output: { file: output },
    watch: {
      exclude: 'main.js',
    },
  });
  onTestFinished(() => watcher.close());

  await waitBuildFinished(watcher);

  // edit file
  await editFile(input, 'console.log(2)');
  // The input is excluded, so the output file should not be updated
  await expect.poll(() => fs.readFileSync(output, 'utf-8')).toContain('console.log(1)');
});

test.sequential('watch onInvalidate', async () => {
  const { input, output } = await createTestInputAndOutput('on-invalidate');

  const onInvalidateFn = vi.fn();
  const watcher = watch({
    input,
    output: { file: output },
    watch: {
      onInvalidate: (id) => {
        expect(id).toBe(input);
        onInvalidateFn(id);
      },
    },
  });
  onTestFinished(() => watcher.close());

  await waitBuildFinished(watcher);

  // edit file
  await editFile(input, 'console.log(2)');

  await expect.poll(() => fs.readFileSync(output, 'utf-8')).toContain('console.log(2)');
  expect(onInvalidateFn).toBeCalled();
});

test.sequential('error handling', async () => {
  // first build error, the watching could be work with recover error
  const { input, output } = await createTestInputAndOutput('error-handling', 'conso le.log(1)');

  const watcher = watch({
    input,
    output: { file: output },
  });
  onTestFinished(() => watcher.close());
  const errors: string[] = [];
  watcher.on('event', (event) => {
    if (event.code === 'ERROR') {
      errors.push(event.error.message);
    }
  });
  // First build should error
  await expect.poll(() => errors.length).toBe(1);
  expect(errors[0]).toContain('PARSE_ERROR');

  await editFile(input, 'console.log(2)');
  await waitBuildFinished(watcher);

  // failed again
  await editFile(input, 'conso le.log(1)');
  // The different platform maybe emit multiple events
  await expect.poll(() => errors.length).toBeGreaterThan(0);
  expect(errors[0]).toContain('PARSE_ERROR');

  // It should be working if the changes are fixed error
  await editFile(input, 'console.log(3)');
  await expect.poll(() => fs.readFileSync(output, 'utf-8')).toContain('console.log(3)');
});

test.sequential('error handling + plugin error', async () => {
  const { input, output } = await createTestInputAndOutput('error-handling-plugin-error');
  const watcher = watch({
    input,
    output: { file: output },
    plugins: [
      {
        name: 'test',
        transform() {
          this.error('plugin error');
        },
      },
    ],
  });
  onTestFinished(() => watcher.close());
  const errors: string[] = [];
  watcher.on('event', (event) => {
    if (event.code === 'ERROR') {
      errors.push(event.error.message);
    }
  });
  // First build should error
  // the revert change maybe emit the change event caused it failed
  await expect.poll(() => errors.length).toBe(1);
  expect(errors[0]).toContain('plugin error');

  errors.length = 0;
  await editFile(input, 'console.log(2)');
  // The different platform maybe emit multiple events
  await expect.poll(() => errors.length).toBeGreaterThan(0);
  expect(errors[0]).toContain('plugin error');
});

test.sequential('watch multiply options', async () => {
  const { input, output, outputDir } = await createTestInputAndOutput('watch-multiply-options');
  const { input: foo, outputDir: fooOutputDir } = await createTestInputAndOutput(
    'watch-multiply-options-foo',
  );
  const watcher = watch([
    {
      input,
      output: { dir: outputDir },
    },
    {
      input: foo,
      output: { dir: fooOutputDir },
    },
  ]);
  onTestFinished(() => watcher.close());

  const events: string[] = [];
  watcher.on('event', (event) => {
    if (event.code === 'BUNDLE_END') {
      events.push(event.output[0]);
    }
  });

  // here should using waitBuildFinished to wait the build finished, because the `input` could be finished before `foo`
  // await waitBuildFinished(watcher)
  await expect.poll(() => fs.readFileSync(output, 'utf-8')).toContain('console.log(1)');

  await editFile(input, 'console.log(2)');
  await expect.poll(() => fs.readFileSync(output, 'utf-8')).toContain('console.log(2)');
  // Only the input corresponding bundler is rebuild
  expect(events[0]).toEqual(outputDir);
});

test.sequential('warning for multiply notify options', async () => {
  const { input, output } = await createTestInputAndOutput('watch-multiply-options-warning');
  const { input: foo } = await createTestInputAndOutput('watch-multiply-options-warning-foo');
  const onLogFn = vi.fn();
  const watcher = watch([
    {
      input,
      output: { file: output },
      watch: {
        watcher: {
          usePolling: true,
          pollInterval: 50,
        },
      },
    },
    {
      input: foo,
      output: { file: output },
      watch: {
        watcher: {
          usePolling: true,
          pollInterval: 100,
        },
      },
      plugins: [
        {
          name: 'test',
          onLog: (level, log) => {
            onLogFn();
            expect(level).toBe('warn');
            expect(log.code).toBe('MULTIPLE_WATCHER_OPTION');
          },
        },
      ],
    },
  ]);
  onTestFinished(() => watcher.close());

  await expect.poll(() => onLogFn).toBeCalled();
});

if (process.platform === 'win32') {
  test.sequential('watch linux path at windows #4385', async () => {
    const { input, output } = await createTestInputAndOutput('watch-linux-path-at-windows');
    const watcher = watch({
      input,
      output: { file: output },
      plugins: [
        {
          name: 'test',
          resolveId() {
            return input.replace(/\\/g, '/');
          },
        },
      ],
    });
    onTestFinished(() => watcher.close());
    // should run build once
    await waitBuildFinished(watcher);

    // edit file
    await editFile(input, 'console.log(2)');
    await expect.poll(() => fs.readFileSync(output, 'utf-8')).toContain('console.log(2)');
  });
}

test.sequential('watch close immediately', async () => {
  const { input, output } = await createTestInputAndOutput('watch-close-immediately');
  const watcher = watch({
    input,
    output: { file: output },
  });

  await watcher.close();
});

test.sequential('ids loaded via load hook should not be watched', async () => {
  const dirname = 'watchFiles-load-hook';
  createTestWithMultiFiles(dirname, {
    'main.js': `import './loaded.js'`,
    'loaded.js': `console.log('on disk')`,
  });
  const cwd = path.join(import.meta.dirname, 'temp', dirname);

  const bundle = await rolldown({
    cwd,
    input: 'main.js',
    plugins: [
      {
        name: 'test-load',
        load(id) {
          if (id.endsWith('loaded.js')) {
            return `console.log('from load hook')`;
          }
        },
      },
    ],
  });
  await bundle.generate();
  const watchFiles = await bundle.watchFiles;
  await bundle.close();

  const normalized = watchFiles.map((f) => f.replace(/\\/g, '/'));
  expect(normalized).toContainEqual(expect.stringContaining('main.js'));
  expect(normalized).not.toContainEqual(expect.stringContaining('loaded.js'));
});

test.sequential('ids loaded by file read should be watched', async () => {
  const dirname = 'watchFiles-file-read';
  createTestWithMultiFiles(dirname, {
    'main.js': `import './dep.js'`,
    'dep.js': `console.log('dep')`,
  });
  const cwd = path.join(import.meta.dirname, 'temp', dirname);

  const bundle = await rolldown({ cwd, input: 'main.js' });
  await bundle.generate();
  const watchFiles = await bundle.watchFiles;
  await bundle.close();

  const normalized = watchFiles.map((f) => f.replace(/\\/g, '/'));
  expect(normalized).toContainEqual(expect.stringContaining('main.js'));
  expect(normalized).toContainEqual(expect.stringContaining('dep.js'));
});

test.sequential('ids added via addWatchFile should be watched', async () => {
  const dirname = 'watchFiles-addWatchFile';
  createTestWithMultiFiles(dirname, {
    'main.js': `console.log('hello')`,
    'external.txt': 'some data',
  });
  const cwd = path.join(import.meta.dirname, 'temp', dirname);
  const externalFile = path.join(cwd, 'external.txt');

  const bundle = await rolldown({
    cwd,
    input: 'main.js',
    plugins: [
      {
        name: 'test-addWatchFile',
        buildStart() {
          this.addWatchFile(externalFile);
        },
      },
    ],
  });
  await bundle.generate();
  const watchFiles = await bundle.watchFiles;
  await bundle.close();

  const normalized = watchFiles.map((f) => f.replace(/\\/g, '/'));
  expect(normalized).toContainEqual(expect.stringContaining('main.js'));
  expect(normalized).toContainEqual(expect.stringContaining('external.txt'));
});

async function createTestInputAndOutput(dirname: string, content?: string) {
  const dir = path.join(import.meta.dirname, 'temp', dirname);
  fs.mkdirSync(dir, { recursive: true });
  const input = path.join(dir, './main.js');
  fs.writeFileSync(input, content || 'console.log(1)');
  await sleep(60); // TODO: find a way to avoid emit the change event at next test
  const outputDir = path.join(dir, './dist');
  const output = path.join(outputDir, 'main.js');
  return { input, output, dir, outputDir };
}

async function createTestWithMultiFiles(dirname: string, files: Record<string, string>) {
  const dir = path.join(import.meta.dirname, 'temp', dirname);
  fs.mkdirSync(dir, { recursive: true });
  for (const [fileName, content] of Object.entries(files)) {
    const filePath = path.join(dir, fileName);
    fs.writeFileSync(filePath, content);
  }
}

async function waitBuildFinished(watcher: RolldownWatcher, updateFn?: () => void) {
  return new Promise<void>((resolve, reject) => {
    let listened = false;
    watcher.on('event', (event) => {
      if (listened) return;

      if (event.code === 'BUNDLE_END') {
        listened = true;
        resolve();
      } else if (event.code === 'ERROR') {
        listened = true;
        reject(event.error);
      }
    });
    updateFn && updateFn();
  });
}
