import { execFileSync } from 'node:child_process';
import { existsSync, readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

import { createBuildCommand, NapiCli } from '@napi-rs/cli';

import {
  beginBuildArtifactTransaction,
  BINDING_BUILD_ARTIFACT_SELECTION,
} from './build-binding-artifacts';
import {
  assertAsyncRuntimeHostExports,
  assertWasiBindingContextLifecycle,
} from './binding-loader-codegen';
import {
  generateWorkerdLoader,
  isAsyncRuntimeDeclarationBuild,
  preserveInactiveWasiDeclaration,
} from './generate-workerd-loader';

const __dirname = dirname(fileURLToPath(import.meta.url));
const WASI_THREADS_TARGET = 'wasm32-wasip1-threads';
const WASI_SINGLE_TARGET = 'wasm32-wasip1';
const WASI_BINARY_NAME = 'rolldown-binding.wasm32-wasi';

const args = process.argv.slice(2);

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
  // napi-rs keys this cache only by crate path and CLI version, so it retains
  // declarations after the Rust binding metadata changes. WASI builds must
  // regenerate their exact declaration surface.
  dtsCache:
    argsOptions.target !== WASI_THREADS_TARGET &&
    argsOptions.target !== WASI_SINGLE_TARGET &&
    !isAsyncRuntimeDeclarationBuild(argsOptions),
  constEnum: false,
};

console.info('args:', napiArgs);

const artifactTransaction = beginBuildArtifactTransaction(
  join(__dirname, 'src'),
  BINDING_BUILD_ARTIFACT_SELECTION,
);
try {
  const restoreInactiveWasiDeclaration = preserveInactiveWasiDeclaration(argsOptions);
  try {
    const { task } = await napiCli.build(napiArgs);
    await task;
  } finally {
    restoreInactiveWasiDeclaration();
  }
  validateWasiBindingContextLifecycles();
  validateAsyncRuntimeHostExports();
  if (argsOptions.target === WASI_THREADS_TARGET) {
    validateWasiReactorArtifacts();
  }
  generateWorkerdLoader();
  artifactTransaction.commit();
} catch (error) {
  console.error(error);
  try {
    artifactTransaction.rollback();
  } catch (rollbackError) {
    console.error(rollbackError);
  }

  process.exit(1);
}

// `napi.wasm.asyncRuntime` makes the cli assert the host contract for the WASI
// loaders it generates (and their own load-time check fails with
// `ERR_NAPI_ASYNC_RUNTIME_BINDING_MISMATCH`). The native loader has no such
// gate, and `src/timer-host.ts` reads the seven exports straight off it.
function validateAsyncRuntimeHostExports(): void {
  assertAsyncRuntimeHostExports(
    readFileSync(join(__dirname, 'src', 'binding.cjs'), 'utf8'),
    'commonjs',
  );
}

function configureWasiRustc(target: unknown): void {
  if (target !== WASI_THREADS_TARGET && target !== WASI_SINGLE_TARGET) return;

  // RUSTC must be the real toolchain binary, not the rustup shim, so the wasi
  // link step can locate crt1-reactor.o (same as reusable-wasi.yml).
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
  const releaseArtifact = join(__dirname, 'src', `${WASI_BINARY_NAME}.wasm`);
  if (!existsSync(releaseArtifact)) {
    throw new Error(`WASI build did not produce ${releaseArtifact}`);
  }

  const debugArtifact = join(__dirname, 'src', `${WASI_BINARY_NAME}.debug.wasm`);
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

function validateWasiBindingContextLifecycles(): void {
  const sourceDir = join(__dirname, 'src');
  for (const bindingPath of [
    join(sourceDir, 'rolldown-binding.wasi.cjs'),
    join(sourceDir, 'rolldown-binding.wasi-browser.js'),
    join(sourceDir, 'rolldown-binding.wasip1.cjs'),
    join(sourceDir, 'rolldown-binding.wasip1-browser.js'),
  ]) {
    assertWasiBindingContextLifecycle(readFileSync(bindingPath, 'utf8'));
  }
}
