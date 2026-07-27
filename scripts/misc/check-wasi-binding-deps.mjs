// Verify that release staging removed the registry runtime dependencies from
// both generated WASI packages after replacing every runtime-bearing loader
// with its self-contained bundle.
//
// Vendoring is load-bearing, not cosmetic: the emnapi v2 plugin exports
// (`emnapiAsyncWorkPlugin` / `emnapiTSFNPlugin`) that the WASI loaders import
// from `@napi-rs/wasm-runtime` currently exist ONLY via a local pnpm patch
// (see the patch note in pnpm-workspace.yaml). pnpm `patchedDependencies` are
// never propagated to registry consumers, so any package that still resolved
// `@napi-rs/wasm-runtime` from the registry would load the pristine runtime,
// which lacks those exports, and fail at load time. Bundling the patched
// runtime into the artifacts is what makes publishing safe while the patch is
// in place — so a registry runtime dependency surviving staging is a release
// blocker, which is exactly what this script asserts.

import * as fs from 'node:fs';
import * as path from 'node:path';
import { fileURLToPath } from 'node:url';

const REPO_ROOT = fileURLToPath(new URL('../../', import.meta.url));
const TRACKED = ['@napi-rs/wasm-runtime', '@emnapi/core', '@emnapi/runtime', 'buffer'];
const BINDING_PKGS = [
  path.join(REPO_ROOT, 'packages/rolldown/npm/wasm32-wasip1/package.json'),
  path.join(REPO_ROOT, 'packages/rolldown/npm/wasm32-wasi/package.json'),
];

let failed = false;
for (const bindingPkg of BINDING_PKGS) {
  const manifest = JSON.parse(fs.readFileSync(bindingPkg, 'utf8'));
  const externalRuntimeDependencies = TRACKED.filter((name) => manifest.dependencies?.[name]);
  if (externalRuntimeDependencies.length > 0) {
    failed = true;
    console.error(
      `${manifest.name} must vendor its runtime but still declares: ${externalRuntimeDependencies.join(', ')}`,
    );
  }
}

if (failed) {
  console.error(
    'Run scripts/misc/stage-wasi-packages.mjs after downloading both bundled loader artifacts.',
  );
  process.exit(1);
}

console.log('OK: both WASI binding packages vendor their runtime dependencies.');
