// Verify that release staging removed the registry runtime dependencies from
// both generated WASI packages after replacing every runtime-bearing loader
// with its self-contained bundle.
//
// Vendoring is load-bearing, not cosmetic: the emnapi v2 plugin exports
// (`emnapiAsyncWorkPlugin` / `emnapiTSFNPlugin`) that the WASI loaders import
// from `@napi-rs/wasm-runtime` ship upstream since the `@napi-rs/cli` 3.8.4 /
// `@napi-rs/wasm-runtime` 1.2.2 pair, so no local pnpm patch is involved
// anymore. The vendored-runtime invariant still holds on its own: staging
// replaces every runtime-bearing loader with a self-contained bundle whose
// embedded runtime is exactly the audited workspace version, so a registry
// runtime dependency surviving staging would reintroduce an unpinned runtime
// resolution the packed artifacts were never validated against. That is a
// release blocker, which is exactly what this script asserts.

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
