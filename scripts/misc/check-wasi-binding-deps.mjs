// Verify that the runtime dependencies of `@rolldown/browser` satisfy the
// version ranges declared by the generated `@rolldown/binding-wasm32-wasi`
// (produced by `napi create-npm-dirs`). The binding is the source of truth:
// its glue code was built against those exact runtime versions, so the
// browser package — which bundles the glue code — must resolve to versions
// compatible with what the binding expects.

import { execFileSync } from 'node:child_process';
import * as fs from 'node:fs';
import * as path from 'node:path';
import { fileURLToPath } from 'node:url';
import semver from 'semver';

const REPO_ROOT = fileURLToPath(new URL('../../', import.meta.url));
const TRACKED = ['@napi-rs/wasm-runtime', '@emnapi/core', '@emnapi/runtime'];
const BINDING_PKG = path.join(REPO_ROOT, 'packages/rolldown/npm/wasm32-wasi/package.json');

function readBindingSpecifiers() {
  const pkg = JSON.parse(fs.readFileSync(BINDING_PKG, 'utf8'));
  const out = {};
  for (const name of TRACKED) {
    const v = pkg.dependencies?.[name];
    if (!v) {
      console.error(`${name} is missing from ${path.relative(REPO_ROOT, BINDING_PKG)}`);
      process.exit(1);
    }
    out[name] = v;
  }
  return out;
}

function readBrowserResolved() {
  const stdout = execFileSync(
    'vp',
    ['pm', 'list', '--filter', '@rolldown/browser', '--json', '--', ...TRACKED],
    { encoding: 'utf8', cwd: REPO_ROOT },
  );
  const parsed = JSON.parse(stdout);
  const deps = parsed[0]?.dependencies ?? {};
  const out = {};
  for (const name of TRACKED) {
    const entry = deps[name];
    if (!entry?.version) {
      console.error(`${name} is not installed under @rolldown/browser — did \`vp install\` run?`);
      process.exit(1);
    }
    out[name] = entry.version;
  }
  return out;
}

const bindingSpecifiers = readBindingSpecifiers();
const browserResolved = readBrowserResolved();

const mismatches = [];
for (const name of TRACKED) {
  const range = bindingSpecifiers[name];
  const version = browserResolved[name];
  if (!semver.satisfies(version, range)) {
    mismatches.push({ name, range, version });
  }
}

if (mismatches.length > 0) {
  console.error(
    "@rolldown/browser's installed runtime deps do not satisfy @rolldown/binding-wasm32-wasi:",
  );
  console.error();
  for (const { name, range, version } of mismatches) {
    console.error(`  ${name}`);
    console.error(`    binding declares: ${range}`);
    console.error(`    browser resolved: ${version}`);
  }
  console.error();
  console.error(
    'The generated binding is the source of truth. Bump the corresponding entry in pnpm-workspace.yaml (if referenced via `catalog:`) or in packages/browser/package.json so the resolved version falls within the binding range.',
  );
  process.exit(1);
}

console.log(
  'OK: @rolldown/browser satisfies @rolldown/binding-wasm32-wasi on @napi-rs/wasm-runtime, @emnapi/core, @emnapi/runtime.',
);
