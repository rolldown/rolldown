import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import { spawn } from 'node:child_process';
import path from 'node:path';

const dist = path.resolve(import.meta.dirname, 'dist');
for (const name of ['main', 'control']) {
  const child = spawn(process.execPath, [path.join(dist, `${name}.js`)], { encoding: 'utf8' });
  let stdout = '';
  let stderr = '';
  child.stdout.on('data', (chunk) => { stdout += chunk; });
  child.stderr.on('data', (chunk) => { stderr += chunk; });
  const code = await new Promise((resolve) => child.on('close', resolve));
  assert.equal(code, 0, `${name} exited ${code}: ${stderr}`);
  assert.equal(stdout.trim(), 'answer: 42', `${name} output`);
}
assert.ok((await readFile(path.join(dist, 'package.json'), 'utf8')).includes('"type": "module"'));
