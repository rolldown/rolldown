import { globSync } from 'glob';
import { spawnSync } from 'node:child_process';
import { rmSync } from 'node:fs';
import { dirname } from 'node:path';
import { fileURLToPath } from 'node:url';

const __dirname = dirname(fileURLToPath(import.meta.url));

const args = process.argv.slice(2);

const napiArgs = [
  'napi',
  'build',
  ...(process.env.CI ? ['--no-dts-cache'] : []),
  '-o=./src',
  '--manifest-path',
  '../../crates/rolldown_binding/Cargo.toml',
  '--platform',
  '-p',
  'rolldown_binding',
  '--js',
  'binding.js',
  '--dts',
  'binding.d.ts',
  '--no-const-enum',
  ...args,
];
console.info('args:', napiArgs);

const cmd = spawnSync(
  'pnpm',
  napiArgs,
  {
    stdio: 'inherit', // Directly inherit stdio (preserves colors)
    env: { ...process.env, RUSTC_COLOR: 'always' }, // Force color output
    shell: true,
    cwd: __dirname,
  },
);

if (cmd.status !== 0) {
  globSync('src/rolldown-binding.*.node', {
    absolute: true,
    cwd: __dirname,
  }).forEach((file) => {
    rmSync(file, { force: true, recursive: true });
  });

  globSync('./src/rolldown-binding.*.wasm', {
    absolute: true,
    cwd: __dirname,
  }).forEach((file) => {
    rmSync(file, { recursive: true, force: true });
  });

  console.error('Command failed!');
  process.exit(cmd.status);
}
