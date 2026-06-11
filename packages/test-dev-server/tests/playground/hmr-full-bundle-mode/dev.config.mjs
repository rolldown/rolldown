import { defineDevConfig } from '@rolldown/test-dev-server';
import nodeFs from 'node:fs';
import nodePath from 'node:path';

export default defineDevConfig({
  platform: 'browser',
  build: {
    input: {
      main: 'main.js',
    },
    platform: 'browser',
    treeshake: false,
    experimental: {
      devMode: {},
    },
    plugins: [
      waitBundleCompleteUntilAccess(),
      delayTransformComment(),
      breakGenerateBundleOnFlag(),
    ],
  },
});

/**
 * Plugin: Wait for first server access before completing bundle
 * Simulates bundle completion timing for testing
 */
function waitBundleCompleteUntilAccess() {
  return {
    name: 'wait-bundle-complete-until-access',

    // Delay bundle generation
    async generateBundle() {
      // Simulate delay for bundle completion timing
      await new Promise((resolve) => setTimeout(resolve, 300));
    },
  };
}

/**
 * Plugin: Delay transform for files with "// @delay-transform" comment
 * Used to test debouncing and concurrent bundle scenarios
 */
function delayTransformComment() {
  return {
    name: 'delay-transform-comment',

    async transform(code, id) {
      if (code.includes('// @delay-transform')) {
        console.log(`[delay-transform] Delaying transform for ${id} by 500ms...`);
        await new Promise((resolve) => setTimeout(resolve, 500));
        console.log(`[delay-transform] Transform complete for ${id}`);
      }
      return null; // No transformation, just delay
    },
  };
}

/**
 * Plugin: fail `generateBundle` unless `rebuild-error/flag.txt` says "ok".
 * The flag file is not watched, so changing it never triggers a build by
 * itself — the failure shows up on the next rebuild (HMR patches don't run
 * generateBundle). The flag text goes into the error message so specs can
 * tell one failing build from the next.
 */
function breakGenerateBundleOnFlag() {
  const flagPath = nodePath.join(import.meta.dirname, 'rebuild-error', 'flag.txt');
  return {
    name: 'break-generate-bundle-on-flag',

    generateBundle() {
      const flag = nodeFs.readFileSync(flagPath, 'utf-8').trim();
      if (flag !== 'ok') {
        throw new Error(`generateBundle broken by flag: ${flag}`);
      }
    },
  };
}
