import assert from 'node:assert';
import fs from 'node:fs';
import path from 'node:path';

const output = fs.readFileSync(path.resolve(import.meta.dirname, 'dist/main.js'), 'utf-8');

const shebang = '#!/usr/bin/env node\n';
const banner = '/* banner */';

assert(
  output.startsWith(shebang + banner),
  `Expected output to start with shebang + banner, but got:\n${output.slice(0, 100)}`,
);
