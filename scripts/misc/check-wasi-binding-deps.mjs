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
const WORKSPACE_MANIFEST = path.join(REPO_ROOT, 'pnpm-workspace.yaml');

// The emnapi v2 plugin exports (`emnapiAsyncWorkPlugin` / `emnapiTSFNPlugin`)
// that @rolldown/browser's loader and the generated @rolldown/binding-wasm32-wasi
// glue import from `@napi-rs/wasm-runtime` currently exist ONLY via a local pnpm
// patch. pnpm `patchedDependencies` are never propagated to registry consumers,
// so a fresh install of either package resolves the pristine
// `@napi-rs/wasm-runtime`, which lacks those exports, and fails at load time
// (browser: a static ESM link error; wasi: undefined plugins at instantiate).
// Refuse to publish while the patch is in place — drop it once a v2-ready
// `@napi-rs/wasm-runtime` is published upstream (see the patch note in
// pnpm-workspace.yaml), which lets these artifacts ship unpatched.
function assertWasmRuntimeNotPatched() {
  const manifest = fs.readFileSync(WORKSPACE_MANIFEST, 'utf8');
  // Matches only the `patchedDependencies` entry — a quoted `@napi-rs/wasm-runtime@<version>`
  // key mapping to a `patches/…` file — not the catalog range or onlyBuiltDependencies list.
  const patched = /^\s*['"]@napi-rs\/wasm-runtime@[^'"]+['"]\s*:\s*patches\//m.test(manifest);
  if (patched) {
    console.error(
      '@napi-rs/wasm-runtime is pnpm-patched to add the emnapi v2 plugin exports\n' +
        '(emnapiAsyncWorkPlugin / emnapiTSFNPlugin). Those exports do NOT ship to registry\n' +
        'consumers, so a fresh install of @rolldown/browser or @rolldown/binding-wasm32-wasi\n' +
        'would load the pristine @napi-rs/wasm-runtime and fail. Refusing to publish.\n\n' +
        'Drop the @napi-rs/wasm-runtime patchedDependencies entry once a v2-ready\n' +
        '@napi-rs/wasm-runtime is published upstream, then re-run.',
    );
    process.exit(1);
  }
}

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

assertWasmRuntimeNotPatched();

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
