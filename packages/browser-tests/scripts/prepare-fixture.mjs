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
// webcontainer-fallback: the same `rolldown` and @rolldown/binding-wasm32-wasi tarballs, for the
//   plain-Node suite that exercises the WebContainer download fallback.
// browser: the same @rolldown/browser tarball, installed into tests/browser, the app the
//   real-browser suite loads through Vite.
import { execFileSync } from 'node:child_process';
import {
  copyFileSync,
  mkdirSync,
  readFileSync,
  readdirSync,
  renameSync,
  rmSync,
  statSync,
  writeFileSync,
} from 'node:fs';
import { basename, dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const packageRoot = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const repoRoot = resolve(packageRoot, '../..');
const fixtures = join(packageRoot, 'tests/fixtures');
const browserPage = join(packageRoot, 'tests/browser');
const rolldownPackage = join(repoRoot, 'packages/rolldown');

const args = process.argv.slice(2);
const shouldBuild = !args.includes('--no-build');
const suite = args.find((arg) => !arg.startsWith('--')) ?? 'all';

if (!['webcontainer', 'webcontainer-fallback', 'browser', 'all'].includes(suite)) {
  throw new Error(
    `Unknown suite "${suite}", expected one of webcontainer, webcontainer-fallback, browser, all`,
  );
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
  stageWorkspaceFileOverrides(fixtureDir);
  pack(rolldownPackage, fixtureDir, /^rolldown-\d.*\.tgz$/, 'rolldown.tgz');
  pack(
    npmDir,
    fixtureDir,
    /^rolldown-binding-wasm32-wasi-\d.*\.tgz$/,
    'rolldown-binding-wasm32-wasi.tgz',
  );
}

// TEMPORARY, goes away with the `file:` overrides in the root pnpm-workspace.yaml (the unreleased
// @emnapi/* pins). The napi cli writes the installed emnapi version into the packed WASI binding's
// dependencies, so a consumer outside the repo resolves it from the registry, which does not have
// it. Copy those tarballs next to the fixtures with an overrides.json mapping each package to its
// copy; the consumers in tests/webcontainer and tests/webcontainer-fallback apply it in the form
// their pnpm reads. With no `file:` override left, this only removes what an earlier run staged.
function stageWorkspaceFileOverrides(fixtureDir) {
  const stageDir = join(fixtureDir, '.napi-validation');
  rmSync(stageDir, { recursive: true, force: true });

  const workspace = readFileSync(join(repoRoot, 'pnpm-workspace.yaml'), 'utf8');
  const block = workspace.match(/^overrides:\n((?:[ \t].*\n|\n)*)/m)?.[1] ?? '';
  const entries = [...block.matchAll(/^ {2}'?([^'\s:#][^'\s:]*)'?: file:(\S+)$/gm)];
  if (entries.length === 0) {
    return;
  }

  mkdirSync(stageDir);
  const overrides = {};
  for (const [, name, file] of entries) {
    copyFileSync(resolve(repoRoot, file), join(stageDir, basename(file)));
    overrides[name] = `file:./.napi-validation/${basename(file)}`;
  }
  writeFileSync(join(stageDir, 'overrides.json'), `${JSON.stringify(overrides, null, 2)}\n`);
  console.log(`[prepare-fixture] staged ${entries.length} workspace file: overrides`);
}

// `--ignore-workspace` keeps this out of the repo's pnpm workspace, so `@rolldown/browser` comes
// from the tarball instead of being linked to packages/browser
function installBrowserPage() {
  run('pnpm', ['install', '--ignore-workspace', '--no-frozen-lockfile'], browserPage);
}

if (suite === 'webcontainer' || suite === 'all') {
  packBrowserPackage();
}

if (suite === 'webcontainer' || suite === 'webcontainer-fallback' || suite === 'all') {
  packNodePackages();
}

if (suite === 'browser' || suite === 'all') {
  packBrowserPackage();
  installBrowserPage();
}
