import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import type { ModuleInfo, RolldownWatcher, RolldownWatcherEvent } from 'rolldown';
import { watch } from 'rolldown';
import { test } from 'vitest';

// https://github.com/rolldown/rolldown/issues/11314
test.for([false, true])(
  'PluginContext.load recovers after a watch transform error (skipWrite: %s)',
  async (skipWrite, { expect, onTestFinished }) => {
    // Given a dependency loaded only through the plugin context, without a graph import racing it.
    const cwd = fs.mkdtempSync(path.join(os.tmpdir(), 'rolldown-watch-load-'));
    const input = path.join(cwd, 'main.js');
    const dependency = path.join(cwd, 'dependency.js');
    fs.writeFileSync(input, 'console.log("entry")');
    fs.writeFileSync(dependency, 'before');
    let loaded: ModuleInfo | undefined;
    const watcher = watch({
      input,
      output: { dir: path.join(cwd, 'dist') },
      watch: {
        skipWrite,
        watcher: { usePolling: true, pollInterval: 50, compareContentsForPolling: true },
      },
      plugins: [
        {
          name: 'inspect-dependency',
          async transform(_code, id) {
            if (!id.endsWith('main.js')) return;
            this.addWatchFile(dependency);
            const resolved = await this.resolve('./dependency.js', id);
            assert(resolved && !resolved.external);
            loaded = await this.load(resolved);
            return loaded?.code;
          },
        },
        {
          name: 'compile-dependency',
          transform(code, id) {
            if (!id.endsWith('dependency.js')) return;
            if (code === 'invalid') this.error('intentional transform failure');
            return `export default ${JSON.stringify(code)};`;
          },
        },
      ],
    });
    onTestFinished(async () => {
      await watcher.close();
      fs.rmSync(cwd, { recursive: true, force: true });
    });
    await waitForBuild(watcher);
    expect(loaded).toMatchObject({ code: 'export default "before";' });

    // When a successful build is followed by a genuine transform error and then a repair.
    const failed = waitForBuild(watcher);
    const failure = expect(failed).rejects.toThrow('intentional transform failure');
    fs.writeFileSync(dependency, 'invalid');
    await failure;

    const repaired = waitForBuild(watcher);
    fs.writeFileSync(dependency, 'repaired');
    await repaired;

    // Then the repaired build completes and load returns this build's transformed module info.
    expect(loaded).toMatchObject({ code: 'export default "repaired";' });
  },
);

function waitForBuild(watcher: RolldownWatcher): Promise<void> {
  return new Promise((resolve, reject) => {
    const listener = async (event: RolldownWatcherEvent) => {
      switch (event.code) {
        case 'BUNDLE_END':
          watcher.off('event', listener);
          await event.result.close();
          resolve();
          break;
        case 'ERROR':
          watcher.off('event', listener);
          await event.result.close();
          reject(event.error);
          break;
        case 'START':
        case 'BUNDLE_START':
        case 'END':
          break;
        default:
          event satisfies never;
      }
    };
    watcher.on('event', listener);
  });
}
