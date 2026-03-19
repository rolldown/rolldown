import { defineDevConfig } from '@rolldown/test-dev-server';

export default defineDevConfig({
  platform: 'browser',
  dev: {
    port: 3636,
  },
  build: {
    input: {
      main: 'main.js',
    },
    platform: 'browser',
    treeshake: false,
    experimental: {
      devMode: {},
    },
    plugins: [waitBundleCompleteUntilAccess(), delayTransformComment()],
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
