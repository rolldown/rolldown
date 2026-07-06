import { execFileSync } from 'node:child_process';
import { existsSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

import { createBuildCommand, NapiCli } from '@napi-rs/cli';
import { globSync } from 'glob';

const __dirname = dirname(fileURLToPath(import.meta.url));
const WASI_THREADS_TARGET = 'wasm32-wasip1-threads';
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
  constEnum: false,
};

console.info('args:', napiArgs);

try {
  const { task } = await napiCli.build(napiArgs);
  await task;
  patchWasiNodeWorkerExecArgv();
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

function configureWasiRustc(target: unknown): void {
  if (target !== WASI_THREADS_TARGET) return;

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

function patchWasiNodeWorkerExecArgv(): void {
  const bindingPath = join(__dirname, 'src', 'rolldown-binding.wasi.cjs');
  const source = readFileSync(bindingPath, 'utf8');
  const helperAnchor = 'const __rootDir = __nodePath.parse(process.cwd()).root\n';
  const workerOptions = `    const worker = new Worker(__nodePath.join(__dirname, 'wasi-worker.mjs'), {
      env: process.env,
    })`;
  if (!source.includes(helperAnchor) || !source.includes(workerOptions)) {
    throw new Error(`Unexpected NAPI-RS WASI loader template in ${bindingPath}`);
  }

  const sanitizer = `const __fileWorkerContextFlagsWithValue = new Set([
  '--eval',
  '-e',
  '--input-type',
  '--print',
  '-p',
  '--run',
])
const __fileWorkerContextFlags = new Set(['--check', '-c', '--interactive', '-i'])

function __sanitizeFileWorkerExecArgv(execArgv) {
  const sanitized = []
  for (let index = 0; index < execArgv.length; index += 1) {
    const argument = execArgv[index]
    const equalsIndex = argument.indexOf('=')
    const flag = equalsIndex === -1 ? argument : argument.slice(0, equalsIndex)
    if (__fileWorkerContextFlagsWithValue.has(flag)) {
      if (equalsIndex === -1) {
        index += 1
      }
      continue
    }
    if (__fileWorkerContextFlags.has(argument)) {
      continue
    }
    sanitized.push(argument)
  }
  return sanitized
}

`;
  const patched = source.replace(helperAnchor, sanitizer + helperAnchor).replace(
    workerOptions,
    `    const worker = new Worker(__nodePath.join(__dirname, 'wasi-worker.mjs'), {
      env: process.env,
      execArgv: __sanitizeFileWorkerExecArgv(process.execArgv),
    })`,
  );
  writeFileSync(bindingPath, patched);
}
