import { execFileSync } from 'node:child_process';
import { existsSync, readFileSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

import { createBuildCommand, NapiCli } from '@napi-rs/cli';

import {
  beginBuildArtifactTransaction,
  BINDING_BUILD_ARTIFACT_SELECTION,
} from './build-binding-artifacts';
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
ensureVendoredEmnapiArchives(argsOptions.target);

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
  patchBindingTargetMetadata();
  patchWasiBindingContextLifecycles();
  patchWasiNodeWorkerExecArgvConfig();
  patchWasiNodeAsyncWorkPoolConfig();
  validateAsyncRuntimeHostExports();
  patchWasiBrowserContextDestroyAwaitConfig();
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

function validateAsyncRuntimeHostExports(): void {
  const sourceDir = join(__dirname, 'src');
  const loaders = [
    ['binding.cjs', 'commonjs'],
    ['rolldown-binding.wasi.cjs', 'commonjs'],
    ['rolldown-binding.wasi-browser.js', 'esm'],
    ['rolldown-binding.wasip1.cjs', 'commonjs'],
    ['rolldown-binding.wasip1-browser.js', 'esm'],
  ] as const;
  for (const [name, format] of loaders) {
    const loaderPath = join(sourceDir, name);
    if (!existsSync(loaderPath)) continue;
    assertAsyncRuntimeHostExports(readFileSync(loaderPath, 'utf8'), format);
  }
}

function ensureVendoredEmnapiArchives(target: unknown): void {
  if (target !== WASI_THREADS_TARGET && target !== WASI_SINGLE_TARGET) return;

  // The emnapi v2 archives napi-build's --export flags require are overlaid
  // onto the installed emnapi package (the published 2.0.0-alpha.2 misses the
  // non-threaded wasm32-wasip1 archive entirely). The overlay also runs from
  // the root postinstall; re-running here covers installs that skipped
  // lifecycle scripts. See vendor/emnapi/README.md.
  execFileSync(process.execPath, [join(__dirname, '..', '..', 'vendor', 'emnapi', 'install.mjs')], {
    stdio: 'inherit',
  });
}

function configureWasiRustc(target: unknown): void {
  if (target !== WASI_THREADS_TARGET && target !== WASI_SINGLE_TARGET) return;

  // See internal-docs/async-runtime/implementation.md.
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
    {
      path: join(sourceDir, 'rolldown-binding.wasip1.cjs'),
      target: 'wasi' as const,
    },
    {
      path: join(sourceDir, 'rolldown-binding.wasip1-browser.js'),
      target: 'wasi' as const,
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
    join(sourceDir, 'rolldown-binding.wasip1.cjs'),
    join(sourceDir, 'rolldown-binding.wasip1-browser.js'),
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
