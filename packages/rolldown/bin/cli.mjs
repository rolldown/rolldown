#!/usr/bin/env node
import module from 'node:module';
try {
  module.enableCompileCache?.();
  setTimeout(() => {
    try {
      module.flushCompileCache?.();
    } catch {}
  }, 10 * 1000).unref();
} catch {}
await import('../dist/cli.mjs');
