import fs from 'node:fs';
import assert from 'node:assert';
import path from 'node:path';

const sourceMap = JSON.parse(
  fs.readFileSync(path.resolve(import.meta.dirname, 'dist/assets/main.js.map'), 'utf8'),
);
assert.ok(
  !sourceMap.sourcesContent || sourceMap.sourcesContent.length === 0,
  'sourcesContent should be empty or absent',
);
