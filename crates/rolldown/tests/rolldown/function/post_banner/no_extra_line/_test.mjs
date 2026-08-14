import assert from 'node:assert';
import fs from 'node:fs';
import path from 'node:path';

const output = fs.readFileSync(path.resolve(import.meta.dirname, 'dist/main.js'), 'utf-8');

// Test that shebang and banner are directly adjacent without blank lines
const lines = output.split('\n');
assert.strictEqual(lines[0], '#!/usr/bin/env node', 'First line should be shebang');
assert.strictEqual(
  lines[1],
  '// Banner comment',
  'Second line should be banner (no blank line in between)',
);

// Ensure no extra blank lines between shebang and banner
const shebangPlusBanner = '#!/usr/bin/env node\n// Banner comment\n';
assert(
  output.startsWith(shebangPlusBanner),
  `Expected no blank line between shebang and banner.\nGot:\n${output.slice(0, 100)}`,
);
