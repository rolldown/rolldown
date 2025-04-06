const { spawn } = require('node:child_process');
const fs = require('fs-extra');
const glob = require('glob');
const path = require('node:path');

const args = process.argv.slice(2);
console.log(`args: `, args);

const cmd = spawn(
  'npx',
  [
    'napi',
    'build',
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
    '--no-dts-cache',
    ...args,
  ],
  {
    stdio: 'inherit', // Directly inherit stdio (preserves colors)
    env: { ...process.env, RUSTC_COLOR: 'always' }, // Force color output
    shell: true,
  },
);

// Inspect exit code
cmd.on('close', (code) => {
  if (code !== 0) {
    const nodeFiles = glob.globSync(
      [path.resolve(__dirname, 'src/rolldown-binding.*.node')],
      {
        absolute: true,
      },
    );
    nodeFiles.forEach((file) => {
      fs.rmSync(file);
    });

    const wasmFiles = glob.globSync(
      [path.resolve(__dirname, './src/rolldown-binding.*.wasm')],
      {
        absolute: true,
      },
    );

    wasmFiles.forEach((file) => {
      fs.rmSync(file);
    });
    console.error('Command failed!');
    process.exit(code);
  }
});
