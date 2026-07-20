import { execFileSync } from 'node:child_process';
import { existsSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

import { createBuildCommand, NapiCli } from '@napi-rs/cli';
import { globSync } from 'glob';

import {
  assertAsyncRuntimeHostExports,
  patchNativeBindingLoader,
  patchWasiBrowserContextDestroyAwait,
  patchWasiBrowserWorkerTerminationAwait,
  patchWasiBindingContextLifecycle,
  patchWasiBindingLoader,
  patchWasiNodeAsyncWorkPoolSize,
  patchWasiNodeWorkerExecArgv,
} from './binding-loader-codegen';

const __dirname = dirname(fileURLToPath(import.meta.url));
const WASI_THREADS_TARGET = 'wasm32-wasip1-threads';
const WASI_BINARY_NAME = 'rolldown-binding.wasm32-wasi';
const WASI_THREADS_DECLARATION = join(__dirname, 'src', 'rolldown-binding.wasi.d.cts');

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
  // napi-rs keys this cache only by crate path and CLI version, so it can
  // retain declarations after their Rust binding metadata changes. Dedicated
  // WASI builds and native async-runtime builds must regenerate their exact
  // declaration surface instead of reusing that feature-blind cache.
  dtsCache:
    argsOptions.target !== WASI_THREADS_TARGET && !isAsyncRuntimeDeclarationBuild(argsOptions),
  constEnum: false,
};

console.info('args:', napiArgs);

try {
  const restoreInactiveWasiDeclaration = preserveInactiveWasiDeclaration(argsOptions);
  try {
    const { task } = await napiCli.build(napiArgs);
    await task;
  } finally {
    restoreInactiveWasiDeclaration();
  }
  patchBindingTargetMetadata();
  patchWasiBindingContextLifecycles();
  patchWasiNodeWorkerExecArgvConfig();
  patchWasiNodeAsyncWorkPoolConfig();
  validateAsyncRuntimeHostExports();
  patchWasiBrowserContextDestroyAwaitConfig();
  if (argsOptions.target === WASI_THREADS_TARGET) {
    validateWasiReactorArtifacts();
  }
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

interface WasiDeclarationBuildOptions {
  target?: string;
  features?: readonly string[];
}

// The test-only runtime probe features add `__rolldownTest*` exports to the
// generated declaration surface. The default build IS the shared runtime, so
// these features are the only remaining native flag that changes the surface.
const RUNTIME_TEST_FEATURES = ['runtime-waker-teardown-test', 'runtime-submission-failure-test'];

/**
 * Whether this build regenerates `binding.d.cts` with the test-only runtime
 * probe surface (`--features runtime-waker-teardown-test` /
 * `runtime-submission-failure-test` on top of the default shared runtime).
 */
function isAsyncRuntimeDeclarationBuild(options: WasiDeclarationBuildOptions): boolean {
  if (options.target === WASI_THREADS_TARGET) return false;
  return (
    options.features?.some((features) =>
      features.split(',').some((feature) => RUNTIME_TEST_FEATURES.includes(feature.trim())),
    ) === true
  );
}

/**
 * The threaded WASI declaration is the only WASI declaration flavor in this
 * tree, and only the threaded WASI build may regenerate it. Every other build
 * reuses its content through the napi-rs CLI's WASI artifact metadata, but a
 * failed or feature-mismatched build must never leave a clobbered declaration
 * behind, so preserve it byte-for-byte and restore it after the build.
 */
function preserveInactiveWasiDeclaration(options: WasiDeclarationBuildOptions): () => void {
  if (options.target === WASI_THREADS_TARGET) {
    return () => {};
  }
  const declaration = readFileSync(WASI_THREADS_DECLARATION);
  return () => writeFileSync(WASI_THREADS_DECLARATION, declaration);
}

function validateAsyncRuntimeHostExports(): void {
  const sourceDir = join(__dirname, 'src');
  const loaders = [
    ['binding.cjs', 'commonjs'],
    ['rolldown-binding.wasi.cjs', 'commonjs'],
    ['rolldown-binding.wasi-browser.js', 'esm'],
  ] as const;
  for (const [name, format] of loaders) {
    const loaderPath = join(sourceDir, name);
    if (!existsSync(loaderPath)) continue;
    assertAsyncRuntimeHostExports(readFileSync(loaderPath, 'utf8'), format);
  }
}

function configureWasiRustc(target: unknown): void {
  if (target !== WASI_THREADS_TARGET) return;

  // The threaded-WASI binding must link Rust's `crt1-reactor.o` and export
  // `_initialize`. napi-build locates that startup object relative to Cargo's
  // `RUSTC`, but task runners may expose a bare command or the rustup proxy
  // instead of the real toolchain executable, so resolve the active compiler
  // before invoking the WASI build.
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

function patchBindingTargetMetadata(): void {
  const sourceDir = join(__dirname, 'src');
  const nativeBindingPath = join(sourceDir, 'binding.cjs');
  const wasiBindings = [
    {
      path: join(sourceDir, 'rolldown-binding.wasi.cjs'),
      target: 'wasi-threads' as const,
    },
    {
      path: join(sourceDir, 'rolldown-binding.wasi-browser.js'),
      target: 'wasi-threads' as const,
    },
  ];

  writeFileSync(
    nativeBindingPath,
    patchNativeBindingLoader(readFileSync(nativeBindingPath, 'utf8')),
  );
  for (const { path: bindingPath, target: wasiTarget } of wasiBindings) {
    if (!existsSync(bindingPath)) continue;
    writeFileSync(
      bindingPath,
      patchWasiBindingLoader(readFileSync(bindingPath, 'utf8'), wasiTarget),
    );
  }
}

function patchWasiBindingContextLifecycles(): void {
  const sourceDir = join(__dirname, 'src');
  for (const bindingPath of [
    join(sourceDir, 'rolldown-binding.wasi.cjs'),
    join(sourceDir, 'rolldown-binding.wasi-browser.js'),
  ]) {
    if (!existsSync(bindingPath)) continue;
    writeFileSync(bindingPath, patchWasiBindingContextLifecycle(readFileSync(bindingPath, 'utf8')));
  }
}

function patchWasiNodeWorkerExecArgvConfig(): void {
  const bindingPath = join(__dirname, 'src', 'rolldown-binding.wasi.cjs');
  writeFileSync(bindingPath, patchWasiNodeWorkerExecArgv(readFileSync(bindingPath, 'utf8')));
}

function patchWasiNodeAsyncWorkPoolConfig(): void {
  const bindingPath = join(__dirname, 'src', 'rolldown-binding.wasi.cjs');
  writeFileSync(bindingPath, patchWasiNodeAsyncWorkPoolSize(readFileSync(bindingPath, 'utf8')));
}

function patchWasiBrowserContextDestroyAwaitConfig(): void {
  const bindingPath = join(__dirname, 'src', 'rolldown-binding.wasi-browser.js');
  let source = readFileSync(bindingPath, 'utf8');
  source = patchWasiBrowserContextDestroyAwait(source);
  source = patchWasiBrowserWorkerTerminationAwait(source);
  writeFileSync(bindingPath, source);
}
