import { spawnSync } from 'node:child_process';
import * as fs from 'node:fs';
import * as path from 'node:path';

fs.rmSync('dist', { recursive: true, force: true });

const command = 'npm'; // Replace with the command you want to execute
const args = ['run', 'build:all']; // Replace with any arguments for the command

spawnSync(command, args, {
  stdio: ['pipe', process.stdout, process.stderr],
  shell: true,
});

fs.writeFileSync(
  path.resolve(import.meta.dirname, 'dist/cjs/package.json'),
  `{
  "type": "commonjs"
}`,
);
