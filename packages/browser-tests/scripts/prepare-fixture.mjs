// Packs the real publishable rolldown artifacts into the WebContainer fixtures.
//
//   node ./scripts/prepare-fixture.mjs [browser|node|all] [--no-build]
//
// Each scenario packs its tarballs into tests/fixtures/<scenario>, which the test mounts as an
// overlay on top of the shared app in tests/fixtures/app.
//
// browser: @rolldown/browser, the single self-contained package the StackBlitz starter uses.
// node:    the plain `rolldown` package plus the separate @rolldown/binding-wasm32-wasi package.
import { execFileSync } from 'node:child_process';
import { copyFileSync, readdirSync, renameSync, rmSync, statSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const packageRoot = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const repoRoot = resolve(packageRoot, '../..');
const fixtures = join(packageRoot, 'tests/fixtures');
const rolldownPackage = join(repoRoot, 'packages/rolldown');

const args = process.argv.slice(2);
const shouldBuild = !args.includes('--no-build');
const scenario = args.find((arg) => !arg.startsWith('--')) ?? 'all';

if (!['browser', 'node', 'all'].includes(scenario)) {
  throw new Error(`Unknown scenario "${scenario}", expected one of browser, node, all`);
}

// files the @rolldown/binding-wasm32-wasi package publishes, produced by `build-binding:wasi`
const WASI_ARTIFACTS = [
  'rolldown-binding.wasm32-wasi.wasm',
  'rolldown-binding.wasi.cjs',
  'rolldown-binding.wasi.d.cts',
  'rolldown-binding.wasi-browser.js',
  'wasi-worker.mjs',
  'wasi-worker-browser.mjs',
];

// TARGET decides which package `build-node` writes into, so an ambient value would silently
// change the artifact shape; every build below sets it explicitly or drops it
function run(command, commandArgs, cwd, target) {
  const env = { ...process.env };
  delete env.TARGET;
  if (target) {
    env.TARGET = target;
  }
  execFileSync(command, commandArgs, { cwd, stdio: 'inherit', env });
}

// `pnpm pack` names the tarball after the package version; the fixtures pin stable names
function pack(sourceDir, fixtureDir, packed, name) {
  const target = join(fixtureDir, name);
  rmSync(target, { force: true });
  run('pnpm', ['pack', '--pack-destination', fixtureDir], sourceDir);

  const produced = readdirSync(fixtureDir).find((file) => packed.test(file));
  if (!produced) {
    throw new Error(`pnpm pack did not produce a ${packed} tarball in ${fixtureDir}`);
  }
  renameSync(join(fixtureDir, produced), target);

  console.log(`[prepare-fixture] ${name}: ${(statSync(target).size / 1024 / 1024).toFixed(2)} MB`);
}

if (scenario === 'browser' || scenario === 'all') {
  if (shouldBuild) {
    run('pnpm', ['run', '--filter', 'rolldown', 'build-browser-pkg:debug'], repoRoot, 'browser');
  }
  pack(
    join(repoRoot, 'packages/browser'),
    join(fixtures, 'browser'),
    /^rolldown-browser-\d.*\.tgz$/,
    'rolldown-browser.tgz',
  );
}

if (scenario === 'node' || scenario === 'all') {
  if (shouldBuild) {
    run('pnpm', ['run', '--filter', 'rolldown', 'build-binding:wasi'], repoRoot);
    // TARGET is dropped so `dist` keeps the published shape, without the wasm inlined
    run('pnpm', ['run', '--filter', 'rolldown', 'build-node'], repoRoot);
  }

  // always regenerated, so the binding package.json cannot keep a version the tarball outgrew
  const npmDir = join(rolldownPackage, 'npm/wasm32-wasi');
  run('pnpm', ['exec', 'napi', 'create-npm-dirs'], rolldownPackage);
  for (const file of WASI_ARTIFACTS) {
    copyFileSync(join(rolldownPackage, 'src', file), join(npmDir, file));
  }

  const fixtureDir = join(fixtures, 'node');
  pack(rolldownPackage, fixtureDir, /^rolldown-\d.*\.tgz$/, 'rolldown.tgz');
  pack(
    npmDir,
    fixtureDir,
    /^rolldown-binding-wasm32-wasi-\d.*\.tgz$/,
    'rolldown-binding-wasm32-wasi.tgz',
  );
}
