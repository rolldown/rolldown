// Shared AST scan for bare emnapi/wasm-runtime/Buffer specifiers, used by the
// WASI staging script and by both packed-consumer gates. Every call site asserts
// the result deep-equals `[]`, so a walker that silently stopped matching would
// make all three publish gates pass vacuously — the positive control at the
// bottom of this module is what keeps that from happening.

import assert from 'node:assert/strict';

import { parse } from 'acorn';

// A bare specifier that survives into a shipped artifact resolves from the
// registry instead of the vendored copy, which breaks the emnapi ABI the .wasm
// links against. Every `@emnapi/*` and `@napi-rs/*` package is forbidden by
// prefix; the list adds the specifiers those bundles reach for that do not fall
// under either namespace (`@napi-rs/wasm-runtime` imports bare
// `@tybys/wasm-util`, and the loaders vendor their own Buffer).
const bundledRuntimePackages = ['@tybys/wasm-util', 'buffer', 'node:buffer'];

export function isBareRuntimeSpecifier(specifier) {
  return (
    /^@(?:emnapi|napi-rs)\//.test(specifier) ||
    bundledRuntimePackages.some(
      (packageName) => specifier === packageName || specifier.startsWith(`${packageName}/`),
    )
  );
}

export function findBareRuntimeImports(code, sourceType) {
  const program = parse(code, { ecmaVersion: 'latest', sourceType, allowHashBang: true });
  const imports = [];
  const pending = [program];

  while (pending.length > 0) {
    const node = pending.pop();
    if (!node || typeof node !== 'object') continue;

    // `export ... from '...'` resolves its specifier exactly like an import.
    if (
      (node.type === 'ImportDeclaration' ||
        node.type === 'ImportExpression' ||
        node.type === 'ExportNamedDeclaration' ||
        node.type === 'ExportAllDeclaration') &&
      typeof node.source?.value === 'string' &&
      isBareRuntimeSpecifier(node.source.value)
    ) {
      imports.push(node.source.value);
    }
    if (
      node.type === 'CallExpression' &&
      node.arguments?.length === 1 &&
      typeof node.arguments[0]?.value === 'string' &&
      isBareRuntimeSpecifier(node.arguments[0].value) &&
      // `__require` (optionally suffixed) is rolldown's own require-of-external
      // interop helper in ESM output — the shape a real externalization
      // regression of the CJS runtime files would produce.
      ((node.callee?.type === 'Identifier' &&
        (node.callee.name === 'require' || /^__require\d*$/.test(node.callee.name))) ||
        (node.callee?.type === 'MemberExpression' &&
          node.callee.object?.type === 'Identifier' &&
          node.callee.object.name === 'require' &&
          node.callee.property?.type === 'Identifier' &&
          node.callee.property.name === 'resolve'))
    ) {
      imports.push(node.arguments[0].value);
    }

    for (const value of Object.values(node)) {
      if (Array.isArray(value)) {
        pending.push(...value);
      } else if (value && typeof value === 'object') {
        pending.push(value);
      }
    }
  }

  return imports.sort((a, b) => a.localeCompare(b));
}

assert.deepEqual(
  findBareRuntimeImports(
    "export { Buffer } from 'node:buffer'; export * from '@emnapi/core';" +
      " import('@napi-rs/wasm-runtime'); require('buffer');" +
      " require.resolve('@tybys/wasm-util'); __require2('@emnapi/runtime');" +
      " notrequire('@emnapi/wasi-threads');",
    'module',
  ),
  [
    '@emnapi/core',
    '@emnapi/runtime',
    '@napi-rs/wasm-runtime',
    '@tybys/wasm-util',
    'buffer',
    'node:buffer',
  ],
  'bare runtime import scan must cover re-exports, dynamic imports, require, require.resolve and rolldown `__require` interop calls while ignoring look-alike callees',
);
