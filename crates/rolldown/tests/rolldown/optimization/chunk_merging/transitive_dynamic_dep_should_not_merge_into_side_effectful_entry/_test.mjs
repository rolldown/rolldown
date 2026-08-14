import assert from 'node:assert';
import { execFileSync } from 'node:child_process';
import path from 'node:path';

const distMain = path.join(import.meta.dirname, 'dist', 'main.js');
const stdout = execFileSync('node', [distMain], { encoding: 'utf-8' });

assert.strictEqual(stdout.trim(), 'dynamic1:shared-b');
