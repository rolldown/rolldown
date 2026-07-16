// TEMPORARY (remove together with vendor/emnapi, see vendor/emnapi/README.md).
//
// Verifies and copies the vendored emnapi v2 archives over the installed
// `node_modules/emnapi/lib` tree:
//
//   - adds the missing `lib/wasm32-wasip1/libemnapi.a`,
//   - replaces `lib/wasm32-wasip1-threads/libemnapi-napi-rs-mt.a` with a
//     build whose `napi_*_env_cleanup_hook` references use the `napi` wasm
//     import module.
//
// Before copying, every entry of `manifest.json` (written by `build.mjs` at
// generation time) is re-checked:
//
//   - the installed emnapi package version is the pinned one,
//   - every npm-shipped file the archives were built from still matches the
//     recorded hash (catches source drift / a republished tarball),
//   - the vendored archives match their recorded hash and member list
//     (catches stale or modified blobs relative to the recorded generation).
//
// The semantic properties of the archives (env/napi import-module split of
// the cleanup hooks) are additionally verified functionally in CI by the
// `Check minimal cleanup-hook imports` step, which links a fresh wasm and
// asserts its import section.
//
// Runs from the repository `postinstall` hook and from
// `packages/rolldown/build-binding.ts` before every WASI build (so the overlay
// exists even when the install ran with scripts disabled).
//
// Adapted for this pnpm workspace from napi-rs's vendor/emnapi/install.mjs
// (rev a9c93cd5): the emnapi package is resolved through @napi-rs/cli's own
// module context — the exact instance whose `lib/` the patched CLI passes to
// the linker as EMNAPI_LINK_DIR — and existing files are unlinked before the
// copy so the overlay never writes through a pnpm store hard link.
import { copyFileSync, existsSync, mkdirSync, readFileSync, rmSync } from 'node:fs';
import { createRequire } from 'node:module';
import { dirname, join } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

import { hashFile, listArchiveMembers } from './integrity.mjs';

const vendorDir = dirname(fileURLToPath(import.meta.url));
const repoRoot = join(vendorDir, '..', '..');

const EXPECTED_EMNAPI_VERSION = '2.0.0-alpha.2';

function resolveEmnapiPackageJson() {
  const bases = [
    // packages/rolldown depends on @napi-rs/cli; the CLI depends on emnapi.
    pathToFileURL(join(repoRoot, 'packages', 'rolldown', 'package.json')).href,
    import.meta.url,
  ];
  for (const base of bases) {
    const req = createRequire(base);
    // Prefer the emnapi instance @napi-rs/cli itself resolves: that is the
    // lib/ directory the patched CLI exports as EMNAPI_LINK_DIR.
    try {
      const cliPackageJson = req.resolve('@napi-rs/cli/package.json');
      return createRequire(cliPackageJson).resolve('emnapi/package.json');
    } catch {}
    try {
      return req.resolve('emnapi/package.json');
    } catch {}
  }
  return undefined;
}

const emnapiPackageJsonPath = resolveEmnapiPackageJson();
if (!emnapiPackageJsonPath) {
  // Not installed (e.g. a partial install); nothing to patch.
  process.exit(0);
}

const emnapiVersion = JSON.parse(readFileSync(emnapiPackageJsonPath, 'utf8')).version;
if (emnapiVersion !== EXPECTED_EMNAPI_VERSION) {
  throw new Error(
    `vendor/emnapi was built from emnapi@${EXPECTED_EMNAPI_VERSION} but emnapi@${emnapiVersion} is installed. ` +
      'If the newer emnapi ships lib/wasm32-wasip1/libemnapi.a and a lib/wasm32-wasip1-threads/libemnapi-napi-rs-mt.a ' +
      'whose napi_*_env_cleanup_hook references use the `napi` import module, delete vendor/emnapi and its callers; ' +
      'otherwise regenerate the archives with vendor/emnapi/build.mjs.',
  );
}

const emnapiRoot = dirname(emnapiPackageJsonPath);
const manifest = JSON.parse(readFileSync(join(vendorDir, 'manifest.json'), 'utf8'));

if (manifest.emnapiVersion !== EXPECTED_EMNAPI_VERSION) {
  throw new Error(
    `vendor/emnapi/manifest.json was generated from emnapi@${manifest.emnapiVersion}, expected ${EXPECTED_EMNAPI_VERSION}. Regenerate with vendor/emnapi/build.mjs.`,
  );
}

// The archives must have been built from exactly the sources the installed
// npm package ships.
for (const [file, integrity] of Object.entries(manifest.sources)) {
  const actual = hashFile(join(emnapiRoot, file));
  if (actual !== integrity) {
    throw new Error(
      `vendor/emnapi: installed emnapi file ${file} does not match the sources the vendored archives were built from ` +
        `(expected ${integrity}, got ${actual}). Regenerate the archives with vendor/emnapi/build.mjs and review the diff.`,
    );
  }
}

const emnapiLib = join(emnapiRoot, 'lib');

for (const [archive, expected] of Object.entries(manifest.archives)) {
  const source = join(vendorDir, archive);
  if (!existsSync(source)) {
    throw new Error(`vendored archive is missing: ${source}`);
  }
  const integrity = hashFile(source);
  if (integrity !== expected.integrity) {
    throw new Error(
      `vendor/emnapi: ${archive} does not match manifest.json ` +
        `(expected ${expected.integrity}, got ${integrity}). Regenerate with vendor/emnapi/build.mjs.`,
    );
  }
  const members = listArchiveMembers(source);
  if (JSON.stringify(members) !== JSON.stringify(expected.members)) {
    throw new Error(
      `vendor/emnapi: ${archive} member list ${JSON.stringify(members)} does not match manifest.json ${JSON.stringify(expected.members)}.`,
    );
  }
  const targetPath = join(emnapiLib, archive);
  mkdirSync(dirname(targetPath), { recursive: true });
  // pnpm hard-links package files from its global content-addressable store;
  // writing through the link would corrupt the store copy for every project
  // on this machine. Unlink first so the overlay gets its own inode.
  rmSync(targetPath, { force: true });
  copyFileSync(source, targetPath);
  console.info(`vendor/emnapi: verified and installed ${archive}`);
}
