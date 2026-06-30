import { rmSync } from 'node:fs';
import { dirname } from 'node:path';
import { fileURLToPath } from 'node:url';

import { createBuildCommand, NapiCli } from '@napi-rs/cli';
import { globSync } from 'glob';

const __dirname = dirname(fileURLToPath(import.meta.url));

const args = process.argv.slice(2);

// EXPERIMENT: share generic instantiations across crates in release builds to cut
// binary size. Without this, generics like the oxc_resolver methods are monomorphized
// once per crate that uses them (cross-crate instantiations aren't shared in optimized
// builds and fat-LTO doesn't merge them), so they're emitted ~8x.
//
// `-Zshare-generics` is unstable, so opt into it via RUSTC_BOOTSTRAP on the pinned stable
// toolchain. Use CARGO_BUILD_RUSTFLAGS (lowest-priority rustflags source) instead of
// RUSTFLAGS so it does NOT clobber the per-target rustflags in .cargo/config.toml
// (crt-static on windows, simd128 on wasm) — those targets keep their flags and build
// unchanged; every other target additionally gets generic sharing.
if (args.includes('--release')) {
  process.env.RUSTC_BOOTSTRAP = '1';
  process.env.CARGO_BUILD_RUSTFLAGS =
    `${process.env.CARGO_BUILD_RUSTFLAGS ?? ''} -Zshare-generics=yes`.trim();
}

const napiCli = new NapiCli();
const buildCommand = createBuildCommand(args);

const argsOptions = buildCommand.getOptions();

const napiArgs = {
  ...argsOptions,
  outputDir: './src',
  manifestPath: '../../crates/rolldown_binding/Cargo.toml',
  platform: true,
  package: 'rolldown_binding',
  jsBinding: 'binding.cjs',
  dts: 'binding.d.cts',
  constEnum: false,
};

console.info('args:', napiArgs);

try {
  const { task } = await napiCli.build(napiArgs);
  await task;
} catch (error) {
  // remove previous build artifacts
  console.error(error);
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

  process.exit(1);
}
