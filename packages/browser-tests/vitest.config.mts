import { createRequire } from 'node:module';
import { playwright } from '@vitest/browser-playwright';
import { defineConfig } from 'vitest/config';
import { vitestWebContainers } from '@webcontainer/test/plugin';

const { version } = createRequire(import.meta.url)('../rolldown/package.json');

// both suites pin the version they actually loaded against the workspace one; `define` is dropped
// for projects, so hand it over through vitest's own channel instead
const provide = () => ({ rolldownVersion: version as string });

// Fresh objects per project: vitest renames a browser project after its instance and writes that
// name back into the instance, so two projects sharing one `instances` array collide on the name.
const browser = () => ({
  enabled: true,
  // CI uses the runner's system Chrome: closer to what real users run, and it skips the browser
  // download. Locally, Playwright's pinned Chromium keeps versions stable.
  provider: playwright({
    launchOptions: { channel: process.env.CI ? 'chrome' : undefined },
  }),
  instances: [{ browser: 'chromium' as const }],
  headless: true,
});

// Both suites drive a browser, but they test different things from different fixtures, so they stay
// separate projects: `--project webcontainer` and `--project browser` never pull in the other's
// globalSetup or its build inputs.
export default defineConfig({
  test: {
    projects: [
      {
        plugins: [vitestWebContainers()],
        test: {
          name: 'webcontainer',
          include: ['tests/webcontainer/*.test.ts'],
          provide: provide(),
          // refuses to run against tarballs that prepare-fixture.mjs has not refreshed
          globalSetup: ['./scripts/check-fixture-freshness.mjs'],
          // booting WebContainer + installing and building inside it is slow
          testTimeout: 120_000,
          hookTimeout: 120_000,
          // WebContainer boots from a CDN, so the network is in the critical path
          retry: process.env.CI ? 2 : 0,
          browser: browser(),
        },
      },
      {
        server: {
          // the wasm binding is built for wasm32-wasip1-threads and allocates a shared memory,
          // which only a cross-origin isolated page may do
          headers: {
            'Cross-Origin-Opener-Policy': 'same-origin',
            'Cross-Origin-Embedder-Policy': 'require-corp',
          },
        },
        optimizeDeps: {
          // prebundling would rewrite the wasm and worker URLs the wasi glue derives from
          // `import.meta.url`, and would resolve the package's imports for it
          exclude: ['@rolldown/browser'],
        },
        test: {
          name: 'browser',
          // the test sits next to tests/browser/package.json, which installs the packed
          // @rolldown/browser, so its bare import resolves through the published `exports` and
          // picks the `browser` condition instead of the workspace source
          include: ['tests/browser/*.test.ts'],
          provide: provide(),
          globalSetup: ['./scripts/check-fixture-freshness.mjs'],
          // instantiating a 29 MB wasm module and booting its worker takes a while
          testTimeout: 120_000,
          hookTimeout: 120_000,
          browser: browser(),
        },
      },
    ],
  },
});
