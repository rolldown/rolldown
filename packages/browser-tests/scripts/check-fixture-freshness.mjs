// vitest globalSetup: refuse to run against fixtures that prepare-fixture.mjs has not refreshed.
//
// The tarballs and the installed page app are gitignored build output. Without this, `vitest run`
// on its own would happily use a tarball packed from an earlier branch and report a green run for
// code it never built.
import { statSync } from 'node:fs';
import { dirname, join, relative, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const packageRoot = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const repoRoot = resolve(packageRoot, '../..');
const fixtures = join(packageRoot, 'tests/fixtures');

const browserTarball = join(fixtures, 'browser/rolldown-browser.tgz');

// separate build steps produce the wasm and the js glue, and the browser suite is mostly about the
// glue, so the tarball has to be newer than both
const BROWSER_TARBALL_CHECKS = [
  {
    artifact: browserTarball,
    source: join(repoRoot, 'packages/browser/dist/rolldown-binding.wasm32-wasi.wasm'),
  },
  {
    artifact: browserTarball,
    source: join(repoRoot, 'packages/browser/dist/index.browser.mjs'),
  },
];

// each artifact must be at least as new as the thing it was produced from
const SUITES = {
  webcontainer: {
    command: 'just test-webcontainer',
    checks: [
      ...BROWSER_TARBALL_CHECKS,
      {
        artifact: join(fixtures, 'node/rolldown.tgz'),
        source: join(repoRoot, 'packages/rolldown/dist/index.mjs'),
      },
      {
        artifact: join(fixtures, 'node/rolldown-binding-wasm32-wasi.tgz'),
        source: join(repoRoot, 'packages/rolldown/src/rolldown-binding.wasm32-wasi.wasm'),
      },
    ],
  },
  browser: {
    command: 'just test-browser',
    checks: [
      ...BROWSER_TARBALL_CHECKS,
      {
        // pnpm rewrites .modules.yaml on every install, so its mtime is when the page app last
        // picked the tarball up
        artifact: join(packageRoot, 'tests/browser/node_modules/.modules.yaml'),
        source: browserTarball,
      },
    ],
  },
};

export function setup(project) {
  // browser mode names projects "<project> (<browser instance>)"
  const [name] = project.name.split(' ');
  const suite = SUITES[name];
  if (!suite) {
    throw new Error(
      `No fixture freshness checks for vitest project "${name}"; add them to ${relative(repoRoot, fileURLToPath(import.meta.url))}.`,
    );
  }

  const fail = (reason) => {
    throw new Error(
      `${reason}\n\nRun \`${suite.command}\`, which builds the artifacts and prepares the fixtures before running the suite.`,
    );
  };

  for (const { artifact, source } of suite.checks) {
    const artifactStat = statSync(artifact, { throwIfNoEntry: false });
    if (!artifactStat) {
      fail(`Missing fixture ${relative(repoRoot, artifact)}.`);
    }

    const sourceStat = statSync(source, { throwIfNoEntry: false });
    if (!sourceStat) {
      fail(`Missing ${relative(repoRoot, source)}, so the fixtures cannot be trusted.`);
    }

    if (artifactStat.mtimeMs < sourceStat.mtimeMs) {
      fail(
        `Stale fixture ${relative(repoRoot, artifact)}: ${relative(repoRoot, source)} changed after it was produced.`,
      );
    }
  }
}
