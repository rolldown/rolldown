// vitest globalSetup: refuse to run against fixtures that prepare-fixture.mjs has not refreshed.
//
// The tarballs are gitignored build output. Without this, `vitest run` on its own would happily
// mount a tarball packed from an earlier branch and report a green run for code it never built.
import { statSync } from 'node:fs';
import { dirname, join, relative, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const packageRoot = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const repoRoot = resolve(packageRoot, '../..');
const fixtures = join(packageRoot, 'tests/fixtures');

// each tarball must be at least as new as the build output it was packed from
const CHECKS = [
  {
    tarball: join(fixtures, 'browser/rolldown-browser.tgz'),
    source: join(repoRoot, 'packages/browser/dist/rolldown-binding.wasm32-wasi.wasm'),
  },
  {
    tarball: join(fixtures, 'node/rolldown.tgz'),
    source: join(repoRoot, 'packages/rolldown/dist/index.mjs'),
  },
  {
    tarball: join(fixtures, 'node/rolldown-binding-wasm32-wasi.tgz'),
    source: join(repoRoot, 'packages/rolldown/src/rolldown-binding.wasm32-wasi.wasm'),
  },
];

function fail(reason) {
  throw new Error(
    `${reason}\n\nRun \`just test-webcontainer\` (or \`pnpm run --filter browser-tests test:webcontainer\`), which builds the artifacts and packs them before running the suite.`,
  );
}

export function setup() {
  for (const { tarball, source } of CHECKS) {
    const tarballStat = statSync(tarball, { throwIfNoEntry: false });
    if (!tarballStat) {
      fail(`Missing fixture tarball ${relative(repoRoot, tarball)}.`);
    }

    const sourceStat = statSync(source, { throwIfNoEntry: false });
    if (!sourceStat) {
      fail(
        `Missing build output ${relative(repoRoot, source)}, so the fixtures cannot be trusted.`,
      );
    }

    if (tarballStat.mtimeMs < sourceStat.mtimeMs) {
      fail(
        `Stale fixture tarball ${relative(repoRoot, tarball)}: ${relative(repoRoot, source)} was rebuilt after it was packed.`,
      );
    }
  }
}
