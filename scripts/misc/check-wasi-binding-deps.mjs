// Verify that the runtime dependencies in the generated
// `@rolldown/binding-wasm32-wasi/package.json` (produced by `napi create-npm-dirs`)
// match the versions used by `@rolldown/browser`, which references them via the
// pnpm catalog. The generated binding is the source of truth: its glue code
// expects those exact runtime versions, so `@rolldown/browser` must bundle
// against the same specifiers.

import * as fs from 'node:fs';
import * as path from 'node:path';
import { fileURLToPath } from 'node:url';

const REPO_ROOT = fileURLToPath(new URL('../../', import.meta.url));
const TRACKED = ['@napi-rs/wasm-runtime', '@emnapi/core', '@emnapi/runtime'];
const BINDING_PKG = path.join(REPO_ROOT, 'packages/rolldown/npm/wasm32-wasi/package.json');
const BROWSER_PKG = path.join(REPO_ROOT, 'packages/browser/package.json');
const WORKSPACE_YAML = path.join(REPO_ROOT, 'pnpm-workspace.yaml');

function readJson(p) {
  return JSON.parse(fs.readFileSync(p, 'utf8'));
}

function readCatalog() {
  const text = fs.readFileSync(WORKSPACE_YAML, 'utf8');
  const lines = text.split('\n');
  const catalog = {};
  let inCatalog = false;
  for (const line of lines) {
    if (/^catalog:\s*$/.test(line)) {
      inCatalog = true;
      continue;
    }
    if (inCatalog) {
      // End of block: next non-indented non-empty line.
      if (line.length > 0 && !/^\s/.test(line)) break;
      const m = line.match(/^\s+(?:'([^']+)'|"([^"]+)"|([^:\s]+))\s*:\s*(.+?)\s*$/);
      if (m) {
        const name = m[1] ?? m[2] ?? m[3];
        const value = m[4].replace(/^['"]|['"]$/g, '');
        catalog[name] = value;
      }
    }
  }
  return catalog;
}

function resolveBrowserSpecifier(pkg, catalog, name) {
  const v = pkg.dependencies?.[name];
  if (!v) {
    throw new Error(`${name} is missing from ${path.relative(REPO_ROOT, BROWSER_PKG)}`);
  }
  if (v === 'catalog:' || v === 'catalog:default') {
    const resolved = catalog[name];
    if (!resolved) {
      throw new Error(`${name} uses catalog but is not defined in pnpm-workspace.yaml`);
    }
    return resolved;
  }
  if (v.startsWith('catalog:')) {
    throw new Error(`Named catalogs are not supported in this check (${name}: ${v})`);
  }
  return v;
}

const binding = readJson(BINDING_PKG);
const browser = readJson(BROWSER_PKG);
const catalog = readCatalog();

const mismatches = [];
for (const name of TRACKED) {
  const bindingVersion = binding.dependencies?.[name];
  if (!bindingVersion) {
    console.error(`${name} is missing from ${path.relative(REPO_ROOT, BINDING_PKG)}`);
    process.exit(1);
  }
  const browserVersion = resolveBrowserSpecifier(browser, catalog, name);
  if (bindingVersion !== browserVersion) {
    mismatches.push({ name, binding: bindingVersion, browser: browserVersion });
  }
}

if (mismatches.length > 0) {
  console.error('Version mismatch between @rolldown/binding-wasm32-wasi and @rolldown/browser:');
  console.error();
  for (const { name, binding, browser } of mismatches) {
    console.error(`  ${name}`);
    console.error(`    binding-wasm32-wasi: ${binding}`);
    console.error(`    browser (catalog):   ${browser}`);
  }
  console.error();
  console.error(
    'The generated binding is the source of truth. Update the `catalog` entries in pnpm-workspace.yaml to match the versions above.',
  );
  process.exit(1);
}

console.log(
  'OK: @rolldown/binding-wasm32-wasi and @rolldown/browser agree on @napi-rs/wasm-runtime, @emnapi/core, @emnapi/runtime.',
);
