#!/usr/bin/env node
import module from 'node:module';
try {
  module.enableCompileCache?.();
} catch {}
await import('../dist/cli.mjs');
