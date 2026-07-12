import { spawnSync } from 'node:child_process';
import { readdirSync, rmSync } from 'node:fs';
import { homedir } from 'node:os';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

import { createBuildCommand, NapiCli } from '@napi-rs/cli';
import { globSync } from 'glob';

const __dirname = dirname(fileURLToPath(import.meta.url));

const args = process.argv.slice(2);

const napiCli = new NapiCli();
const buildCommand = createBuildCommand(args);

const argsOptions = buildCommand.getOptions();

const isRelease = argsOptions.release === true || argsOptions.profile === 'release';

// For published binaries, remap the absolute build-machine paths (cargo/rustup homes and
// the workspace root) that rustc embeds into panic locations and tracing callsite metadata.
// This shrinks the binary's string tables and keeps the build machine's filesystem layout
// out of the shipped artifact. Release-only so local dev backtraces keep clickable paths.
// Replace with cargo `-Ztrim-paths` once it stabilizes (rust-lang/cargo#12137).
//
// The flags are injected as a cargo `--config target.'cfg(all())'.rustflags=[…]` entry, NOT
// via RUSTFLAGS/CARGO_BUILD_RUSTFLAGS: config-level target rustflags are joined with the
// `.cargo/config.toml` target entries (windows crt-static, ucrt link-args), whereas the napi
// CLI promotes CARGO_BUILD_RUSTFLAGS to RUSTFLAGS, which would suppress those entries
// entirely (measured: it silently dropped crt-static from the windows binary).
// Known gap: napi-cli always sets RUSTFLAGS for musl targets (`-C target-feature=-crt-static`),
// which suppresses config-level rustflags there — musl artifacts keep unremapped paths.
let remapConfig: string | undefined;
if (isRelease) {
  const cargoHome = process.env.CARGO_HOME ?? resolve(homedir(), '.cargo');
  const rustupHome = process.env.RUSTUP_HOME ?? resolve(homedir(), '.rustup');
  const workspaceRoot = resolve(__dirname, '../..');
  const remaps = [
    `--remap-path-prefix=${cargoHome}=/cargo`,
    `--remap-path-prefix=${rustupHome}=/rustup`,
    `--remap-path-prefix=${workspaceRoot}=/rolldown`,
  ];
  // Collapse the long per-registry hash directory (`registry/src/index.crates.io-<hash>`)
  // too: rustc uses the last matching prefix, so these more-specific mappings go last.
  // The registry extraction dirs only exist after dependencies are fetched, and this script
  // runs before napi invokes cargo — on a cold CI runner the directory would be empty. Fetch
  // first (cheap: the same download cargo would do anyway), then enumerate sorted so the
  // resulting flag set is deterministic.
  spawnSync(
    'cargo',
    ['fetch', '--locked', ...(argsOptions.target ? ['--target', argsOptions.target] : [])],
    { cwd: workspaceRoot, stdio: 'inherit' },
  );
  const registrySrc = resolve(cargoHome, 'registry', 'src');
  try {
    for (const dir of readdirSync(registrySrc).sort()) {
      remaps.push(`--remap-path-prefix=${resolve(registrySrc, dir)}=/deps`);
    }
  } catch {
    // no registry dir (e.g. vendored deps) — nothing to collapse
  }
  // TOML literal strings cannot contain single quotes; such paths just skip the remap.
  if (remaps.every((flag) => !flag.includes("'"))) {
    remapConfig = `target.'cfg(all())'.rustflags=[${remaps.map((flag) => `'${flag}'`).join(',')}]`;
  }
}

const napiArgs = {
  ...argsOptions,
  // `getOptions()` doesn't surface CLI rest args, so this doesn't overwrite anything.
  ...(remapConfig ? { cargoOptions: ['--config', remapConfig] } : {}),
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
