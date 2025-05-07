import { spawnSync } from 'node:child_process';
import * as fs from 'node:fs';

fs.rmSync('dist', { recursive: true, force: true });

const command = 'npm'; // Replace with the command you want to execute
const args = ['run', 'build:all']; // Replace with any arguments for the command

spawnSync(command, args, {
  stdio: ['pipe', process.stdout, process.stderr],
});

fs.writeFileSync(
  'dist/cjs/package.json',
  `{
  "type": "commonjs"
}`,
);
