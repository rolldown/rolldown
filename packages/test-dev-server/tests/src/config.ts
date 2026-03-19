import nodeAssert from 'node:assert';
import nodeFs from 'node:fs';
import nodePath from 'node:path';

import { getDevWatchOptionsForCi } from '@rolldown/test-dev-server';

// `/packages/test-dev-server/tests`
const testsDir = nodePath.resolve(import.meta.dirname, '..').normalize();
nodeAssert.ok(nodeFs.existsSync(nodePath.join(testsDir, 'playground')));

export const CONFIG = {
  paths: {
    testsDir,
    playgroundDir: nodePath.join(testsDir, 'playground'),
    tmpPlaygroundDir: nodePath.join(testsDir, 'tmp-playground'),
    hmrFullBundleModeDir: nodePath.join(testsDir, 'playground/hmr-full-bundle-mode'),
    tmpFullBundleModeDir: nodePath.join(testsDir, 'tmp-playground/hmr-full-bundle-mode'),
    lazyCompilationDir: nodePath.join(testsDir, 'playground/lazy-compilation'),
    tmpLazyCompilationDir: nodePath.join(testsDir, 'tmp-playground/lazy-compilation'),
  },
  ports: {
    hmrFullBundleMode: 3636,
    lazyCompilation: 3637,
  },
  watch: getDevWatchOptionsForCi(),
};
