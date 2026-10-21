import { execFileSync } from 'node:child_process';
import { existsSync, readFileSync, readdirSync, rmSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

import { createBuildCommand, NapiCli } from '@napi-rs/cli';
import { globSync } from 'glob';

const __dirname = dirname(fileURLToPath(import.meta.url));
const SOURCE_DIR = join(__dirname, 'src');
const WASI_THREADS_TARGET = 'wasm32-wasip1-threads';
const WASI_SINGLE_TARGET = 'wasm32-wasip1';
const WASI_BINARY_NAME = 'rolldown-binding.wasm32-wasi';
const PRESERVE_FLAG = '--preserve-generated-sources';

const args = process.argv.slice(2).filter((arg) => arg !== PRESERVE_FLAG);

const napiCli = new NapiCli();
const buildCommand = createBuildCommand(args);

const argsOptions = buildCommand.getOptions();
configureWasiRustc(argsOptions.target);

const napiArgs = {
  ...argsOptions,
  outputDir: './src',
  manifestPath: '../../crates/rolldown_binding/Cargo.toml',
  platform: true,
  package: 'rolldown_binding',
  jsBinding: 'binding.cjs',
  dts: 'binding.d.cts',
  // The napi-rs cache key covers build inputs (target, profile, features,
  // flags, cargo config, dependency graph) but never the Rust source, so the
  // cache can retain declarations after the Rust binding metadata changes.
  // WASI builds must regenerate their exact declaration surface.
  dtsCache: argsOptions.target !== WASI_THREADS_TARGET && argsOptions.target !== WASI_SINGLE_TARGET,
  constEnum: false,
};

console.info('args:', napiArgs);

// A test-feature build (`just build-rolldown-async-runtime`) adds test-only
// exports to the generated sources; keep the committed ones.
const restoreGeneratedSources = process.argv.includes(PRESERVE_FLAG)
  ? snapshotGeneratedSources()
  : () => {};
try {
  const { task } = await napiCli.build(napiArgs);
  await task;
  if (argsOptions.target === WASI_THREADS_TARGET) {
    validateWasiReactorArtifacts();
  }
} catch (error) {
  // remove previous build artifacts
  console.error(error);
  for (const file of globSync('src/rolldown-binding.*.{node,wasm}', {
    absolute: true,
    cwd: __dirname,
  })) {
    rmSync(file, { force: true, recursive: true });
  }
  process.exitCode = 1;
} finally {
  restoreGeneratedSources();
}

function isGeneratedSource(name: string): boolean {
  return (
    name === 'browser.js' ||
    /^(?:binding(?:\.d)?|rolldown-binding\..+|wasi-worker(?:-browser)?)\.(?:cjs|cts|js|mjs|ts)(?:\.map)?$/.test(
      name,
    )
  );
}

function snapshotGeneratedSources(): () => void {
  const list = () => readdirSync(SOURCE_DIR).filter(isGeneratedSource);
  const sources = new Map(list().map((name) => [name, readFileSync(join(SOURCE_DIR, name))]));
  return () => {
    for (const name of list()) {
      if (!sources.has(name)) rmSync(join(SOURCE_DIR, name), { force: true });
    }
    for (const [name, source] of sources) {
      writeFileSync(join(SOURCE_DIR, name), source);
    }
  };
}

function configureWasiRustc(target: unknown): void {
  if (target !== WASI_THREADS_TARGET && target !== WASI_SINGLE_TARGET) return;

  // RUSTC must be the real toolchain binary, not the rustup shim, so the wasi
  // link step can locate crt1-reactor.o.
  const rustcPath = resolveRustcPath();
  if (!existsSync(rustcPath)) {
    throw new Error(`Could not resolve the real rustc executable at ${rustcPath}`);
  }
  process.env.RUSTC = rustcPath;
}

function resolveRustcPath(): string {
  try {
    return execFileSync('rustup', ['which', 'rustc'], {
      encoding: 'utf8',
      stdio: ['ignore', 'pipe', 'ignore'],
    }).trim();
  } catch {
    const rustcCommand = process.env.RUSTC || 'rustc';
    const sysroot = execFileSync(rustcCommand, ['--print', 'sysroot'], {
      encoding: 'utf8',
    }).trim();
    return join(sysroot, 'bin', process.platform === 'win32' ? 'rustc.exe' : 'rustc');
  }
}

function validateWasiReactorArtifacts(): void {
  const releaseArtifact = join(SOURCE_DIR, `${WASI_BINARY_NAME}.wasm`);
  if (!existsSync(releaseArtifact)) {
    throw new Error(`WASI build did not produce ${releaseArtifact}`);
  }

  const debugArtifact = join(SOURCE_DIR, `${WASI_BINARY_NAME}.debug.wasm`);
  for (const artifact of [releaseArtifact, debugArtifact]) {
    if (!existsSync(artifact)) continue;
    const module = new WebAssembly.Module(new Uint8Array(readFileSync(artifact)));
    const exports = WebAssembly.Module.exports(module);
    const hasInitialize = exports.some(
      ({ name, kind }) => name === '_initialize' && kind === 'function',
    );
    const hasStart = exports.some(({ name }) => name === '_start');
    if (!hasInitialize || hasStart) {
      throw new Error(
        `WASI reactor invariant failed for ${artifact}: expected a function export named "_initialize" and no "_start" export`,
      );
    }
  }
}
