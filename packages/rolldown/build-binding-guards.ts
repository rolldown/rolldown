/// <reference types="node" />

/**
 * Guards `build-binding.ts` wraps the `@napi-rs/cli` build with. The cli owns
 * every generated loader — including the deferred workerd loader
 * (`rolldown-binding.wasip1-deferred.js`), whose managed facade lives in
 * `src/workerd-managed-instance.ts`. What stays here is the build-time glue
 * around it: keeping the inactive flavor's declaration, restoring generated
 * sources after a throwaway binding build, and validating that the configured
 * threadless memory can actually satisfy the wasm artifact's `env.memory`.
 *
 * See internal-docs/async-runtime/implementation.md.
 */

import { spawnSync } from 'node:child_process';
import { existsSync, readFileSync, readdirSync, rmSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const __dirname = dirname(fileURLToPath(import.meta.url));
const SOURCE_DIR = join(__dirname, 'src');
const WASM_FILENAME = 'rolldown-binding.wasm32-wasip1.wasm';
const WASM_PATH = join(__dirname, 'src', WASM_FILENAME);
const WASM32_MAX_PAGES = 65_536;
const WASI_DECLARATIONS = {
  threaded: join(__dirname, 'src/rolldown-binding.wasi.d.cts'),
  threadless: join(__dirname, 'src/rolldown-binding.wasip1.d.cts'),
} as const;

interface PackageWasmConfig {
  initialMemory?: number;
  threadlessInitialMemory?: number;
  maximumMemory?: number;
}

export interface WasmConfig {
  initialMemory: number;
  maximumMemory: number;
}

export interface WasiDeclarationBuildOptions {
  target?: string;
}

export interface WasiDeclarationPaths {
  threaded: string;
  threadless: string;
}

function isGeneratedBindingSource(name: string): boolean {
  return (
    name === 'browser.js' ||
    /^(?:binding(?:\.d)?|rolldown-binding\..+|wasi-worker(?:-browser)?)\.(?:cjs|cts|js|mjs|ts)(?:\.map)?$/.test(
      name,
    )
  );
}

function listGeneratedBindingSources(sourceDir: string): string[] {
  return readdirSync(sourceDir, { withFileTypes: true })
    .filter((entry) => entry.isFile() && isGeneratedBindingSource(entry.name))
    .map((entry) => entry.name);
}

export async function preserveGeneratedBindingSources<T>(
  operation: () => T | Promise<T>,
  sourceDir: string = SOURCE_DIR,
): Promise<T> {
  const sources = new Map(
    listGeneratedBindingSources(sourceDir).map((name) => [
      name,
      readFileSync(join(sourceDir, name)),
    ]),
  );
  try {
    return await operation();
  } finally {
    for (const name of listGeneratedBindingSources(sourceDir)) {
      if (!sources.has(name)) {
        rmSync(join(sourceDir, name), { force: true });
      }
    }
    for (const [name, source] of sources) {
      writeFileSync(join(sourceDir, name), source);
    }
  }
}

export function isAsyncRuntimeDeclarationBuild(options: WasiDeclarationBuildOptions): boolean {
  return options.target === 'wasm32-wasip1';
}

function getActiveWasiDeclarationFlavor(
  options: WasiDeclarationBuildOptions,
): keyof WasiDeclarationPaths {
  return isAsyncRuntimeDeclarationBuild(options) ? 'threadless' : 'threaded';
}

export function preserveInactiveWasiDeclaration(
  options: WasiDeclarationBuildOptions,
  paths: WasiDeclarationPaths = WASI_DECLARATIONS,
): () => void {
  const activeFlavor = getActiveWasiDeclarationFlavor(options);
  const inactiveFlavor = activeFlavor === 'threadless' ? 'threaded' : 'threadless';
  const inactivePath = paths[inactiveFlavor];
  const declaration = readFileSync(inactivePath);
  return () => writeFileSync(inactivePath, declaration);
}

function validateMemoryPages(value: number, field: string): number {
  if (!Number.isSafeInteger(value) || value < 1 || value > WASM32_MAX_PAGES) {
    throw new TypeError(`${field} must be a positive integer no greater than ${WASM32_MAX_PAGES}`);
  }
  return value;
}

function readUnsignedLeb128(bytes: Uint8Array, offset: { value: number }, label: string): number {
  let value = 0;
  let shift = 0;
  while (offset.value < bytes.length) {
    const byte = bytes[offset.value];
    offset.value += 1;
    value += (byte & 0x7f) * 2 ** shift;
    if (!Number.isSafeInteger(value)) {
      throw new RangeError(`${label} exceeds JavaScript's safe integer range`);
    }
    if ((byte & 0x80) === 0) return value;
    shift += 7;
    if (shift > 49) break;
  }
  throw new Error(`Malformed ${label} in ${WASM_FILENAME}`);
}

function readWasmName(
  bytes: Uint8Array,
  offset: { value: number },
  sectionEnd: number,
  label: string,
): string {
  const length = readUnsignedLeb128(bytes, offset, `${label} length`);
  const end = offset.value + length;
  if (end > sectionEnd) {
    throw new Error(`Malformed ${label} in ${WASM_FILENAME}`);
  }
  const value = Buffer.from(bytes.subarray(offset.value, end)).toString('utf8');
  offset.value = end;
  return value;
}

function readImportedMemoryLimits(): { minimum: number; maximum?: number } {
  const bytes = new Uint8Array(readFileSync(WASM_PATH));
  if (
    bytes.length < 8 ||
    bytes[0] !== 0x00 ||
    bytes[1] !== 0x61 ||
    bytes[2] !== 0x73 ||
    bytes[3] !== 0x6d ||
    bytes[4] !== 0x01 ||
    bytes[5] !== 0x00 ||
    bytes[6] !== 0x00 ||
    bytes[7] !== 0x00
  ) {
    throw new Error(`${WASM_FILENAME} is not a WebAssembly 1.0 module`);
  }

  const offset = { value: 8 };
  const memoryImports: Array<{
    module: string;
    name: string;
    minimum: number;
    maximum?: number;
  }> = [];
  while (offset.value < bytes.length) {
    const sectionId = bytes[offset.value];
    offset.value += 1;
    const sectionSize = readUnsignedLeb128(bytes, offset, 'section size');
    const sectionEnd = offset.value + sectionSize;
    if (sectionEnd > bytes.length) {
      throw new Error(`Malformed section ${sectionId} in ${WASM_FILENAME}`);
    }
    if (sectionId !== 2) {
      offset.value = sectionEnd;
      continue;
    }

    const importCount = readUnsignedLeb128(bytes, offset, 'import count');
    for (let index = 0; index < importCount; index += 1) {
      const module = readWasmName(bytes, offset, sectionEnd, 'import module');
      const name = readWasmName(bytes, offset, sectionEnd, 'import name');
      if (offset.value >= sectionEnd) {
        throw new Error(`Malformed import ${module}.${name} in ${WASM_FILENAME}`);
      }
      const kind = bytes[offset.value];
      offset.value += 1;
      if (kind === 0) {
        readUnsignedLeb128(bytes, offset, `function import ${module}.${name}`);
        continue;
      }
      if (kind !== 2) {
        throw new Error(
          `Unsupported non-function import ${module}.${name} (kind ${kind}) in ${WASM_FILENAME}`,
        );
      }

      const flags = readUnsignedLeb128(bytes, offset, `memory import ${module}.${name} flags`);
      if ((flags & ~0x01) !== 0) {
        throw new Error(
          `The threadless loader requires an unshared memory32 import; ${module}.${name} has flags ${flags}`,
        );
      }
      const minimum = readUnsignedLeb128(bytes, offset, `memory import ${module}.${name} minimum`);
      const maximum =
        (flags & 0x01) === 0
          ? undefined
          : readUnsignedLeb128(bytes, offset, `memory import ${module}.${name} maximum`);
      memoryImports.push({ module, name, minimum, maximum });
    }
    if (offset.value !== sectionEnd) {
      throw new Error(`Malformed import section in ${WASM_FILENAME}`);
    }
    break;
  }

  if (
    memoryImports.length !== 1 ||
    memoryImports[0].module !== 'env' ||
    memoryImports[0].name !== 'memory'
  ) {
    throw new Error(
      `${WASM_FILENAME} must import exactly one memory as env.memory; found ${memoryImports
        .map(({ module, name }) => `${module}.${name}`)
        .join(', ')}`,
    );
  }
  return memoryImports[0];
}

/**
 * Validate the configured threadless memory window against the wasm artifact's
 * own `env.memory` limits. The cli compiles `napi.wasm.threadlessInitialMemory`
 * into the loaders it generates, so a config below the module's minimum would
 * ship a loader that cannot instantiate.
 */
export function assertThreadlessMemoryConfig(): WasmConfig {
  const packageJson = JSON.parse(readFileSync(join(__dirname, 'package.json'), 'utf8')) as {
    napi?: { wasm?: PackageWasmConfig };
  };
  const wasm = packageJson.napi?.wasm;
  const initialMemory = validateMemoryPages(
    wasm?.threadlessInitialMemory ?? wasm?.initialMemory ?? 4000,
    'napi.wasm.threadlessInitialMemory',
  );
  const maximumMemory = validateMemoryPages(
    wasm?.maximumMemory ?? 65536,
    'napi.wasm.maximumMemory',
  );
  if (initialMemory > maximumMemory) {
    throw new RangeError(
      'napi.wasm.threadlessInitialMemory must not exceed napi.wasm.maximumMemory',
    );
  }
  // Native builds also run this post-generator, but the ignored threadless
  // Wasm artifact is absent in a clean native checkout.
  if (existsSync(WASM_PATH)) {
    const importedMemory = readImportedMemoryLimits();
    if (initialMemory < importedMemory.minimum) {
      throw new RangeError(
        `napi.wasm.threadlessInitialMemory (${initialMemory}) is below ${WASM_FILENAME}'s env.memory minimum (${importedMemory.minimum})`,
      );
    }
    if (importedMemory.maximum !== undefined && maximumMemory > importedMemory.maximum) {
      throw new RangeError(
        `napi.wasm.maximumMemory (${maximumMemory}) exceeds ${WASM_FILENAME}'s env.memory maximum (${importedMemory.maximum})`,
      );
    }
  }
  return { initialMemory, maximumMemory };
}

if (process.argv[1] && fileURLToPath(import.meta.url) === process.argv[1]) {
  if (process.argv[2] === '--preserve-generated-sources') {
    const commandArgs = process.argv.slice(3);
    if (commandArgs[0] === '--') commandArgs.shift();
    const command = commandArgs.shift();
    if (!command) {
      throw new Error('Expected a command after --preserve-generated-sources --');
    }
    let result: ReturnType<typeof spawnSync> | undefined;
    await preserveGeneratedBindingSources(() => {
      result = spawnSync(command, commandArgs, {
        cwd: __dirname,
        env: process.env,
        stdio: 'inherit',
      });
    });
    if (result?.error) throw result.error;
    if (result?.signal) {
      throw new Error(`Preserved binding build terminated by signal ${result.signal}`);
    }
    process.exitCode = result?.status ?? 1;
  } else {
    assertThreadlessMemoryConfig();
  }
}
