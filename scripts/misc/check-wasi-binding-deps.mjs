// Verify that release staging removed the registry runtime dependencies from
// both generated WASI packages after replacing every runtime-bearing loader
// with its self-contained bundle.

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
