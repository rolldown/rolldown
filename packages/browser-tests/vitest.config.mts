import { createRequire } from 'node:module';
import { playwright } from '@vitest/browser-playwright';
import { defineConfig } from 'vitest/config';
import { vitestWebContainers } from '@webcontainer/test/plugin';

const { version } = createRequire(import.meta.url)('../rolldown/package.json');

export default defineConfig({
  plugins: [vitestWebContainers()],
  define: {
    __ROLLDOWN_VERSION__: JSON.stringify(version),
  },
  test: {
    include: ['tests/*.test.ts'],
    exclude: ['tests/fixtures/**'],
    // refuses to run against tarballs that prepare-fixture.mjs has not refreshed
    globalSetup: ['./scripts/check-fixture-freshness.mjs'],
    // booting WebContainer + installing and building inside it is slow
    testTimeout: 120_000,
    hookTimeout: 120_000,
    // WebContainer boots from a CDN, so the network is in the critical path
    retry: process.env.CI ? 2 : 0,
    browser: {
      enabled: true,
      provider: playwright({
        // CI uses the runner's system Chrome: closer to what real StackBlitz users run, and it
        // skips the browser download. Locally, Playwright's pinned Chromium keeps versions stable.
        launchOptions: { channel: process.env.CI ? 'chrome' : undefined },
      }),
      instances: [{ browser: 'chromium' }],
      headless: true,
    },
  },
});
