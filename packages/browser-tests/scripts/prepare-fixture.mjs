// Packs the real publishable rolldown artifacts into the fixtures each test suite consumes.
//
//   node ./scripts/prepare-fixture.mjs [webcontainer|browser|all] [--no-build]
//
// `webcontainer` and `browser` are the vitest project names, so this takes the same word as
// `vitest run --project <name>`.
//
// webcontainer: the tarballs tests/webcontainer mounts inside the container.
//   - @rolldown/browser, the single self-contained package the StackBlitz starter uses.
//   - the plain `rolldown` package plus the separate @rolldown/binding-wasm32-wasi package.
// browser: the same @rolldown/browser tarball, installed into tests/browser, the app the
//   real-browser suite loads through Vite.
import { execFileSync } from 'node:child_process';
import { copyFileSync, readdirSync, renameSync, rmSync, statSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const packageRoot = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const repoRoot = resolve(packageRoot, '../..');
const fixtures = join(packageRoot, 'tests/fixtures');
const browserPage = join(packageRoot, 'tests/browser');
const rolldownPackage = join(repoRoot, 'packages/rolldown');

const args = process.argv.slice(2);
const shouldBuild = !args.includes('--no-build');
const suite = args.find((arg) => !arg.startsWith('--')) ?? 'all';

if (!['webcontainer', 'browser', 'all'].includes(suite)) {
  throw new Error(`Unknown suite "${suite}", expected one of webcontainer, browser, all`);
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

// shared by both suites, so pack it once even when preparing everything
let browserPacked = false;
function packBrowserPackage() {
  if (browserPacked) {
    return;
  }
  browserPacked = true;

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

function packNodePackages() {
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

// `--ignore-workspace` keeps this out of the repo's pnpm workspace, so `@rolldown/browser` comes
// from the tarball instead of being linked to packages/browser
function installBrowserPage() {
  run('pnpm', ['install', '--ignore-workspace', '--no-frozen-lockfile'], browserPage);
}

if (suite === 'webcontainer' || suite === 'all') {
  packBrowserPackage();
  packNodePackages();
}

if (suite === 'browser' || suite === 'all') {
  packBrowserPackage();
  installBrowserPage();
}
