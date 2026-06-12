import assert from 'node:assert';
import { execFileSync } from 'node:child_process';
import { readdirSync, readFileSync } from 'node:fs';
import path from 'node:path';

const distDir = path.join(import.meta.dirname, 'dist');
const files = readdirSync(distDir)
  .filter((file) => file.endsWith('.js'))
  .sort();

assert.deepStrictEqual(files, [
  'main.js',
  'main2.js',
  'rolldown-runtime.js',
  'shared-abc~a.js',
  'shared-abc~b.js',
  'shared-abc~c.js',
]);

const main = readFileSync(path.join(distDir, 'main.js'), 'utf8');
assert.match(main, /import\("\.\/shared-abc~a\.js"\)\.then/);
assert.match(main, /import\("\.\/shared-abc~b\.js"\)\.then/);
assert.match(main, /import\("\.\/shared-abc~c\.js"\)\.then/);
assert.doesNotMatch(main, /import\("\.\/[abc]\.js"\)/);

const main2 = readFileSync(path.join(distDir, 'main2.js'), 'utf8');
assert.match(main2, /import\("\.\/shared-abc~a\.js"\)\.then/);
assert.match(main2, /import\("\.\/shared-abc~b\.js"\)\.then/);
assert.doesNotMatch(main2, /import\("\.\/shared-abc~c\.js"\)/);
assert.doesNotMatch(main2, /import\("\.\/[abc]\.js"\)/);

execFileSync('node', [path.join(distDir, 'main.js')], { encoding: 'utf-8' });
execFileSync('node', [path.join(distDir, 'main2.js')], { encoding: 'utf-8' });
