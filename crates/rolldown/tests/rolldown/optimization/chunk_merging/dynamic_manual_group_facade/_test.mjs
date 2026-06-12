import assert from 'node:assert';
import { execFileSync } from 'node:child_process';
import { readdirSync, readFileSync } from 'node:fs';
import path from 'node:path';

const distDir = path.join(import.meta.dirname, 'dist');
const files = readdirSync(distDir).filter((file) => file.endsWith('.js'));

assert.deepStrictEqual(files.sort(), ['main.js', 'shared-abc.js']);

const main = readFileSync(path.join(distDir, 'main.js'), 'utf8');
assert.match(main, /import\("\.\/shared-abc\.js"\)\.then/);
assert.doesNotMatch(main, /import\("\.\/[abc]\.js"\)/);

const distMain = path.join(distDir, 'main.js');
execFileSync('node', [distMain], { encoding: 'utf-8' });
