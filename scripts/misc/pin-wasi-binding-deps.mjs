// `napi create-npm-dirs` gives `@rolldown/binding-wasm32-wasi` the range `~<latest>` of
// `@napi-rs/wasm-runtime`. Each wasm-runtime release loads with only one `@emnapi/core` 2.0
// prerelease, so a newer patch breaks the emnapi version the binding ships. Write the catalog
// version instead, and bump it together with emnapi. Run it right after `napi create-npm-dirs`.

import * as fs from 'node:fs';
import * as path from 'node:path';
import { fileURLToPath } from 'node:url';

const REPO_ROOT = fileURLToPath(new URL('../../', import.meta.url));
const NAME = '@napi-rs/wasm-runtime';
const BINDING_PKG = path.join(REPO_ROOT, 'packages/rolldown/npm/wasm32-wasi/package.json');

const workspace = fs.readFileSync(path.join(REPO_ROOT, 'pnpm-workspace.yaml'), 'utf8');
const version = /^ {2}'@napi-rs\/wasm-runtime': (\S+)$/m.exec(workspace)?.[1];
if (!version) {
  console.error(`${NAME} is missing from the catalog in pnpm-workspace.yaml`);
  process.exit(1);
}

const pkg = JSON.parse(fs.readFileSync(BINDING_PKG, 'utf8'));
if (!pkg.dependencies?.[NAME]) {
  console.error(`${NAME} is missing from ${path.relative(REPO_ROOT, BINDING_PKG)}`);
  process.exit(1);
}
pkg.dependencies[NAME] = version;
fs.writeFileSync(BINDING_PKG, `${JSON.stringify(pkg, null, 2)}\n`);
console.log(`Pinned ${NAME} to ${version} in ${pkg.name}.`);
