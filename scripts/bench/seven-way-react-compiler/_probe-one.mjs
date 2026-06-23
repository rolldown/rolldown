#!/usr/bin/env node
// Bundle one file with builtin's React Compiler config and report PANIC/ERROR.

import { rolldown } from 'rolldown';
import { readFileSync } from 'node:fs';

const absPath = process.argv[2];
if (!absPath) {
  console.error('usage: _probe-one.mjs <abs-path>');
  process.exit(2);
}

const ENTRY_ID = '\0probe:entry';
const entrySource = `import ${JSON.stringify(absPath)};`;

try {
  const bundle = await rolldown({
    input: ENTRY_ID,
    plugins: [
      {
        name: 'virtual-entry',
        resolveId(id) {
          if (id === ENTRY_ID) return id;
          if (id && !id.startsWith('.') && !id.startsWith('/') && !id.startsWith('\0')) return { id, external: true };
          if (id && /\.(css|svg|png|jpg|jpeg|gif|woff2?|ttf|otf|md)$/.test(id)) return { id, external: true };
          return null;
        },
        load(id) { if (id === ENTRY_ID) return entrySource; return null; },
      },
    ],
    transform: { reactCompiler: { panicThreshold: 'none' } },
    logLevel: 'silent',
    onLog() {},
    shimMissingExports: true,
  });
  await bundle.generate({ format: 'esm' });
  await bundle.close();
  console.log('OK');
} catch (e) {
  // Check for the oxc panic in the error chain
  const msg = String(e?.message ?? e ?? '');
  const stack = String(e?.stack ?? '');
  const errs = Array.isArray(e?.errors) ? JSON.stringify(e.errors).slice(0, 500) : '';
  if (msg.includes('unreachable') || stack.includes('unreachable') || errs.includes('unreachable')) {
    console.log('PANIC');
  } else {
    console.log('ERROR');
  }
}
