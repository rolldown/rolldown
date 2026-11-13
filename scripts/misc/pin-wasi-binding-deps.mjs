// `napi create-npm-dirs` gives both WASI bindings (`@rolldown/binding-wasm32-wasi` and
// `@rolldown/binding-wasm32-wasip1`) the range `~<latest>` of `@napi-rs/wasm-runtime`. Each
// wasm-runtime release loads with only one `@emnapi/core` 2.0 prerelease, so a newer patch breaks
// the emnapi version the binding ships. Write the catalog version instead, and bump it together
// with emnapi. Run it right after `napi create-npm-dirs`.

import * as fs from 'node:fs';
import * as path from 'node:path';
import { fileURLToPath } from 'node:url';

const REPO_ROOT = fileURLToPath(new URL('../../', import.meta.url));
const NAME = '@napi-rs/wasm-runtime';
const BINDING_PKGS = ['wasm32-wasi', 'wasm32-wasip1'].map((dir) =>
  path.join(REPO_ROOT, 'packages/rolldown/npm', dir, 'package.json'),
);

const workspace = fs.readFileSync(path.join(REPO_ROOT, 'pnpm-workspace.yaml'), 'utf8');
const version = /^ {2}'@napi-rs\/wasm-runtime': (\S+)$/m.exec(workspace)?.[1];
if (!version) {
  console.error(`${NAME} is missing from the catalog in pnpm-workspace.yaml`);
  process.exit(1);
}

for (const bindingPkg of BINDING_PKGS) {
  const pkg = JSON.parse(fs.readFileSync(bindingPkg, 'utf8'));
  if (!pkg.dependencies?.[NAME]) {
    console.error(`${NAME} is missing from ${path.relative(REPO_ROOT, bindingPkg)}`);
    process.exit(1);
  }
  pkg.dependencies[NAME] = version;
  fs.writeFileSync(bindingPkg, `${JSON.stringify(pkg, null, 2)}\n`);
  console.log(`Pinned ${NAME} to ${version} in ${pkg.name}.`);
}
