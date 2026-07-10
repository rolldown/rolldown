/// <reference types="node" />

import { spawnSync } from 'node:child_process';
import { existsSync, readFileSync, readdirSync, rmSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const __dirname = dirname(fileURLToPath(import.meta.url));
const SOURCE_DIR = join(__dirname, 'src');
const DEFERRED_LOADER = join(__dirname, 'src/rolldown-binding.wasip1-deferred.js');
const DEFERRED_LOADER_DECLARATION = join(__dirname, 'src/rolldown-binding.wasip1-deferred.d.ts');
const WASM_FILENAME = 'rolldown-binding.wasm32-wasip1.wasm';
const WASM_PATH = join(__dirname, 'src', WASM_FILENAME);
const WASM32_MAX_PAGES = 65_536;
const CURRENT_THREAD_TASK_HOST_CONTRACT_VERSION = 4;
const WASI_DECLARATIONS = {
  threaded: join(__dirname, 'src/rolldown-binding.wasi.d.cts'),
  threadless: join(__dirname, 'src/rolldown-binding.wasip1.d.cts'),
} as const;
// See internal-docs/async-runtime/implementation.md.
const CURRENT_THREAD_HOST_BOOTSTRAP_START = '/* ROLLDOWN_CURRENT_THREAD_HOST_BOOTSTRAP_START */';
const CURRENT_THREAD_HOST_BOOTSTRAP_END = '/* ROLLDOWN_CURRENT_THREAD_HOST_BOOTSTRAP_END */';
const BROWSER_INITIALIZATION_GUARD_START = '/* ROLLDOWN_BROWSER_INITIALIZATION_GUARD_START */';
const BROWSER_INITIALIZATION_GUARD_END = '/* ROLLDOWN_BROWSER_INITIALIZATION_GUARD_END */';
const NODE_INITIALIZATION_CLEANUP_START = '/* ROLLDOWN_NODE_INITIALIZATION_CLEANUP_START */';
const NODE_INITIALIZATION_CLEANUP_END = '/* ROLLDOWN_NODE_INITIALIZATION_CLEANUP_END */';
const NODE_LIFECYCLE_HELPER_SIGNATURES = [
  'function __destroyEmnapiContext() {',
  'function __removeEmnapiContextBeforeExitListener() {',
  'function __removeEmnapiContextAtExitListener() {',
  'function __removeEmnapiContextCleanupListeners() {',
  'function __retainEmnapiContextCleanupListener() {',
  'function __handoffEmnapiContextCleanupToExit() {',
  'function __attachCleanupError(__error, __cleanupError) {',
  'function __preserveCleanupError(__error, __cleanupError) {',
] as const;
const NODE_LIFECYCLE_CONTRACT_SNIPPETS = [
  `function __removeEmnapiContextBeforeExitListener() {
  if (__emnapiContextRegisteredForBeforeExit) {
    process.removeListener('beforeExit', __destroyEmnapiContextBeforeExit)
    __emnapiContextRegisteredForBeforeExit = false
  }
}`,
  `function __removeEmnapiContextAtExitListener() {
  if (__emnapiContextRegisteredForExit) {
    process.removeListener('exit', __destroyEmnapiContextAtExit)
    __emnapiContextRegisteredForExit = false
  }
}`,
  `function __removeEmnapiContextCleanupListeners() {
  const __errors = []
  try {
    __removeEmnapiContextBeforeExitListener()
  } catch (__error) {
    __errors.push(__error)
  }
  try {
    __removeEmnapiContextAtExitListener()
  } catch (__error) {
    __errors.push(__error)
  }
  if (__errors.length === 0) {
    __emnapiContextBeforeExitRegistrationRetryCount = 0
    return
  }
  if (__errors.length === 1) {
    throw __errors[0]
  }
  throw new AggregateError(
    __errors,
    'emnapi context cleanup listener removal failed',
  )
}`,
  `function __retainEmnapiContextCleanupListener() {
  if (
    __emnapiContextDestroyed ||
    __emnapiContextRegisteredForBeforeExit ||
    __emnapiContextRegisteredForExit
  ) {
    return
  }
  __registerEmnapiContextBeforeExit()
}`,
  `function __handoffEmnapiContextCleanupToExit() {
  const __exitWasRegistered = __emnapiContextRegisteredForExit
  __registerEmnapiContextAtExit()
  try {
    __removeEmnapiContextBeforeExitListener()
  } catch (__error) {
    if (!__exitWasRegistered) {
      try {
        __removeEmnapiContextAtExitListener()
      } catch (__rollbackError) {
        throw new AggregateError(
          [__error, __rollbackError],
          'emnapi context cleanup listener handoff failed',
          { cause: __error },
        )
      }
    }
    throw __error
  }
}`,
  `function __attachCleanupError(__error, __cleanupError) {
  try {
    if (
      __error &&
      (typeof __error === 'object' || typeof __error === 'function')
    ) {
      if (__error.cause === undefined) {
        __error.cause = __cleanupError
        return __error.cause === __cleanupError
      }
      return __error.cause === __cleanupError
    }
  } catch {}
  return false
}

function __preserveCleanupError(__error, __cleanupError) {
  if (!__attachCleanupError(__error, __cleanupError)) {
    queueMicrotask(() => {
      throw __cleanupError
    })
  }
}`,
  `  __handoffEmnapiContextCleanupToExit()`,
] as const;
const CURRENT_THREAD_LOADERS = [
  {
    path: join(__dirname, 'src/rolldown-binding.wasip1.cjs'),
    bootstrapAnchor: '} catch (__error) {\n  let __cleanupResult\n  let __cleanupFailed = false',
    browserInitializationGuard: false,
  },
  {
    path: join(__dirname, 'src/rolldown-binding.wasip1-browser.js'),
    bootstrapAnchor: 'export default __napiModule.exports',
    browserInitializationGuard: true,
  },
] as const;

interface PackageWasmConfig {
  initialMemory?: number;
  threadlessInitialMemory?: number;
  maximumMemory?: number;
}

interface WasmConfig {
  initialMemory: number;
  maximumMemory: number;
}

export interface WasiDeclarationBuildOptions {
  target?: string;
  features?: readonly string[];
  noDefaultFeatures?: boolean;
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
  if (options.target === 'wasm32-wasip1') return true;
  if (options.target === 'wasm32-wasip1-threads') return false;
  return (
    options.noDefaultFeatures === true &&
    options.features?.some((features) =>
      features.split(',').some((feature) => feature.trim() === 'async-runtime'),
    ) === true
  );
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

function readMemoryConfig(): WasmConfig {
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

function renderWorkerdLoader({ initialMemory, maximumMemory }: WasmConfig): string {
  return `import {
  getDefaultContext as __emnapiGetDefaultContext,
  instantiateNapiModule as __emnapiInstantiateNapiModule,
  WASI as __WASI,
} from '@napi-rs/wasm-runtime'
import { createContext as __emnapiCreateContext } from '@emnapi/runtime'
// oxlint-disable-next-line unicorn/prefer-node-protocol -- workerd builds alias this bare specifier to the npm polyfill
import { Buffer } from 'buffer'

export const WORKERD_WASM_MEMORY = Object.freeze({
  initialPages: ${initialMemory},
  maximumPages: ${maximumMemory},
  pageBytes: 65536,
  initialBytes: ${initialMemory} * 65536,
  maximumBytes: ${maximumMemory} * 65536,
})

let __createdManagedInstances = 0
let __liveManagedInstances = 0
// Duplicate bundles pin an opaque monotonic claim operation on an extensible
// Memory object. A non-extensible Memory already has an immutable prototype,
// so its immediate prototype is a stable fallback host. Buffer replacement and
// prototype changes therefore cannot select fresh claim state.
const __managedMemoryClaimsKey = Symbol.for(
  '@rolldown/browser/workerd/managed-memory-claims/v1',
)
const __arrayBufferByteLengthDescriptor = Object.getOwnPropertyDescriptor(
  ArrayBuffer.prototype,
  'byteLength',
)
const __arrayBufferByteLengthGetter = Reflect.get(
  __arrayBufferByteLengthDescriptor,
  'get',
  __arrayBufferByteLengthDescriptor,
)
const __sharedArrayBufferByteLengthDescriptor =
  typeof SharedArrayBuffer === 'function'
    ? Object.getOwnPropertyDescriptor(
        SharedArrayBuffer.prototype,
        'byteLength',
      )
    : undefined
const __sharedArrayBufferByteLengthGetter =
  __sharedArrayBufferByteLengthDescriptor === undefined
    ? undefined
    : Reflect.get(
        __sharedArrayBufferByteLengthDescriptor,
        'get',
        __sharedArrayBufferByteLengthDescriptor,
      )
const __prototypeTraversalLimit = 256
const __managedPrivateBindingExports = new Set([
  'getCurrentThreadTaskHostContractVersion',
  'isCurrentThreadHostRegistrationActive',
  'registerCurrentThreadTaskHost',
  'registerTimerHost',
  'reserveCurrentThreadHostRegistration',
  'unregisterCurrentThreadTaskHost',
  'unregisterTimerHost',
])
const __disposedBindingMessage = 'This workerd Rolldown instance has been disposed'
const __disposingBindingMessage =
  'This workerd Rolldown instance is unavailable because disposal has started'

/**
 * Loader-local diagnostics for managed instances created through this module
 * evaluation. WebAssembly memory
 * byteLength is address-space size, not Cloudflare's committed-memory metric;
 * inspect each handle's memoryBytes alongside platform telemetry.
 */
export function getDeferredRuntimeStats() {
  return Object.freeze({
    createdInstances: __createdManagedInstances,
    liveInstances: __liveManagedInstances,
    declaredInitialMemoryBytes: WORKERD_WASM_MEMORY.initialBytes,
  })
}

function __validateModule(__module) {
  try {
    WebAssembly.Module.imports(__module)
  } catch {
    throw new TypeError(
      "Expected a precompiled WebAssembly.Module (or a Promise resolving to one), " +
        "e.g. import mod from './${WASM_FILENAME}' under a CompiledWasm module rule. " +
        "Byte buffers, URLs and Response objects require dynamic Wasm compilation, which workerd disallows.",
    )
  }
}

function __validateMemory(__memory) {
  const __buffer = __readMemoryBuffer(__memory)
  try {
    Reflect.apply(__arrayBufferByteLengthGetter, __buffer, [])
  } catch {
    throw new TypeError('The threadless workerd loader requires an unshared WebAssembly.Memory')
  }
}

function __hasIntrinsicArrayBufferBrand(__value) {
  try {
    Reflect.apply(__arrayBufferByteLengthGetter, __value, [])
    return true
  } catch {}
  if (__sharedArrayBufferByteLengthGetter !== undefined) {
    try {
      Reflect.apply(__sharedArrayBufferByteLengthGetter, __value, [])
      return true
    } catch {}
  }
  return false
}

function __readMemoryBuffer(__memory) {
  try {
    return Object.getOwnPropertyDescriptor(
      WebAssembly.Memory.prototype,
      'buffer',
    ).get.call(__memory)
  } catch {
    throw new TypeError('memory must be an unshared WebAssembly.Memory')
  }
}

function __assertBoundedMemoryPrototypeChain(__memory) {
  const __seen = new Set()
  let __candidate
  try {
    __candidate = Object.getPrototypeOf(__memory)
  } catch {}
  let __depth = 0
  while (__candidate !== null && __candidate !== undefined) {
    if (__seen.has(__candidate)) {
      throw new TypeError('Cyclic prototype chain detected on managed workerd memory')
    }
    if (__depth >= __prototypeTraversalLimit) {
      throw new TypeError('Managed workerd memory prototype chain exceeds the traversal limit')
    }
    __seen.add(__candidate)
    __depth += 1
    try {
      __candidate = Object.getPrototypeOf(__candidate)
    } catch {
      break
    }
  }
}

function __createManagedMemoryClaim() {
  const __claims = new WeakSet()
  return (__memory) => {
    if (WeakSet.prototype.has.call(__claims, __memory)) return false
    WeakSet.prototype.add.call(__claims, __memory)
    return true
  }
}

function __getManagedMemoryClaim(__memory) {
  __assertBoundedMemoryPrototypeChain(__memory)
  const __readClaim = (__host) => {
    const __descriptor = Object.getOwnPropertyDescriptor(
      __host,
      __managedMemoryClaimsKey,
    )
    if (__descriptor === undefined) return
    const __claim = __descriptor.value
    if (
      __descriptor.configurable ||
      __descriptor.enumerable ||
      __descriptor.writable ||
      typeof __claim !== 'function'
    ) {
      throw new TypeError(
        'The managed workerd memory claim registry is incompatible',
      )
    }
    return __claim
  }
  const __pinnedClaim = __readClaim(__memory)
  if (__pinnedClaim !== undefined) return __pinnedClaim

  let __registryHost = __memory
  if (!Reflect.isExtensible(__memory)) {
    try {
      __registryHost = Object.getPrototypeOf(__memory)
    } catch {}
  }
  if (
    (__registryHost === null ||
      (typeof __registryHost !== 'object' && typeof __registryHost !== 'function'))
  ) {
    throw new TypeError(
      'Unable to safely locate the managed workerd memory claim registry host',
    )
  }

  let __claim = __readClaim(__registryHost)
  if (__claim === undefined) {
    __claim = __createManagedMemoryClaim()
    let __installed = false
    try {
      __installed = Reflect.defineProperty(__registryHost, __managedMemoryClaimsKey, {
        value: __claim,
        configurable: false,
        enumerable: false,
        writable: false,
      })
    } catch {}
    if (!__installed) {
      throw new TypeError(
        'Unable to safely establish the managed workerd memory claim registry',
      )
    }
  }
  return __claim
}

function __claimManagedMemoryForAttempt(__memory) {
  const __claim = __getManagedMemoryClaim(__memory)
  let __claimed
  let __repeated
  try {
    __claimed = Reflect.apply(__claim, undefined, [__memory])
    __repeated = Reflect.apply(__claim, undefined, [__memory])
  } catch {
    throw new TypeError(
      'The managed workerd memory claim registry is incompatible',
    )
  }
  if (__claimed === false && __repeated === false) {
    throw new TypeError(
      'This WebAssembly.Memory has already been used for a managed workerd initialization attempt and cannot be reused, including after failed initialization or disposal',
    )
  }
  if (__claimed !== true || __repeated !== false) {
    throw new TypeError(
      'The managed workerd memory claim registry is incompatible',
    )
  }
}

function __readManagedHostRegistration(__registration) {
  let __high
  let __low
  try {
    __high = Reflect.get(__registration, 'high', __registration)
    __low = Reflect.get(__registration, 'low', __registration)
  } catch {}
  if (
    !Number.isInteger(__high) ||
    __high < 0 ||
    __high > 0xffff_ffff ||
    !Number.isInteger(__low) ||
    __low < 0 ||
    __low > 0xffff_ffff ||
    (__high === 0 && __low === 0)
  ) {
    throw new TypeError('The managed workerd binding returned an invalid host registration')
  }
  return [__high, __low]
}

function __assertManagedHostRegistrationActive(
  __binding,
  __isActive,
  __registration,
  __label,
) {
  const __active = Reflect.apply(__isActive, __binding, __registration)
  if (typeof __active !== 'boolean') {
    throw new TypeError(
      'The managed workerd binding returned an invalid ' +
        __label +
        ' host liveness result',
    )
  }
  if (!__active) {
    throw new TypeError(
      'The managed workerd binding returned an inactive ' +
        __label +
        ' host registration',
    )
  }
}

function __registerManagedCurrentThreadTaskHost(__binding, __captureDisposer) {
  const __getContractVersion = Reflect.get(
    __binding,
    'getCurrentThreadTaskHostContractVersion',
  )
  const __isActive = Reflect.get(
    __binding,
    'isCurrentThreadHostRegistrationActive',
  )
  const __register = Reflect.get(__binding, 'registerCurrentThreadTaskHost')
  const __reserve = Reflect.get(__binding, 'reserveCurrentThreadHostRegistration')
  const __unregister = Reflect.get(__binding, 'unregisterCurrentThreadTaskHost')
  if (
    typeof __getContractVersion !== 'function' ||
    typeof __isActive !== 'function' ||
    typeof __register !== 'function' ||
    typeof __reserve !== 'function' ||
    typeof __unregister !== 'function'
  ) {
    throw new TypeError('The managed workerd binding does not support CurrentThread task hosting')
  }
  const __actualVersion = Reflect.apply(__getContractVersion, __binding, [])
  if (__actualVersion !== ${CURRENT_THREAD_TASK_HOST_CONTRACT_VERSION}) {
    throw new TypeError(
      'The managed workerd binding uses CurrentThread task-host contract version ' +
        String(__actualVersion) +
        ', but version ${CURRENT_THREAD_TASK_HOST_CONTRACT_VERSION} is required',
    )
  }

  let __disposed = false
  const __registration = __readManagedHostRegistration(
    Reflect.apply(__reserve, __binding, []),
  )
  const __dispose = () => {
    if (__disposed) return
    Reflect.apply(__unregister, __binding, __registration)
    __disposed = true
  }
  __captureDisposer(__dispose)
  Reflect.apply(__register, __binding, __registration)
  __assertManagedHostRegistrationActive(
    __binding,
    __isActive,
    __registration,
    'task',
  )
}

function __registerManagedTimerHost(__binding, __captureDisposer) {
  const __setTimeoutHost = globalThis.setTimeout?.bind(globalThis)
  const __clearTimeoutHost = globalThis.clearTimeout?.bind(globalThis)
  if (!__setTimeoutHost || !__clearTimeoutHost) return

  const __getContractVersion = Reflect.get(
    __binding,
    'getCurrentThreadTaskHostContractVersion',
  )
  const __isActive = Reflect.get(
    __binding,
    'isCurrentThreadHostRegistrationActive',
  )
  const __register = Reflect.get(__binding, 'registerTimerHost')
  const __reserve = Reflect.get(__binding, 'reserveCurrentThreadHostRegistration')
  const __unregister = Reflect.get(__binding, 'unregisterTimerHost')
  if (
    typeof __getContractVersion !== 'function' ||
    typeof __isActive !== 'function' ||
    typeof __register !== 'function' ||
    typeof __reserve !== 'function' ||
    typeof __unregister !== 'function'
  ) {
    throw new TypeError('The managed workerd binding does not support timer hosting')
  }
  const __actualVersion = Reflect.apply(__getContractVersion, __binding, [])
  if (__actualVersion !== ${CURRENT_THREAD_TASK_HOST_CONTRACT_VERSION}) {
    throw new TypeError(
      'The managed workerd binding uses CurrentThread task-host contract version ' +
        String(__actualVersion) +
        ', but version ${CURRENT_THREAD_TASK_HOST_CONTRACT_VERSION} is required',
    )
  }
  const __maxHostTimeoutMs = 2_147_483_647
  const __active = new Map()
  let __disposed = false
  const __registration = __readManagedHostRegistration(
    Reflect.apply(__reserve, __binding, []),
  )

  const __armTimer = (__timer, __id) => {
    const __delay = Math.min(__timer.remainingMs, __maxHostTimeoutMs)
    __timer.handle = __setTimeoutHost(() => {
      if (__active.get(__id) !== __timer) return
      __timer.remainingMs -= __delay
      if (__timer.remainingMs > 0) {
        try {
          __armTimer(__timer, __id)
        } catch (__error) {
          __active.delete(__id)
          __timer.reject(__error)
        }
        return
      }
      __active.delete(__id)
      __timer.resolve()
    }, __delay)
  }
  const __cancelTimer = (__timer) => {
    try {
      if (__timer.handle !== undefined) {
        __clearTimeoutHost(__timer.handle)
      }
    } catch {
    } finally {
      __timer.resolve()
    }
  }
  const __schedule = (__id, __ms) => {
    if (__disposed) return Promise.resolve()
    const __previous = __active.get(__id)
    if (__previous) {
      __active.delete(__id)
      __cancelTimer(__previous)
    }
    return new Promise((__resolve, __reject) => {
      const __timer = {
        handle: undefined,
        remainingMs: Math.max(__ms, 0),
        reject: __reject,
        resolve: __resolve,
      }
      __active.set(__id, __timer)
      try {
        __armTimer(__timer, __id)
      } catch (__error) {
        if (__active.get(__id) === __timer) {
          __active.delete(__id)
        }
        __reject(__error)
      }
    })
  }
  const __cancelTimerById = (__id) => {
    const __timer = __active.get(__id)
    if (!__timer) return
    __active.delete(__id)
    __cancelTimer(__timer)
  }
  const __dispose = () => {
    if (__disposed) return
    Reflect.apply(__unregister, __binding, __registration)
    __disposed = true
    const __timers = [...__active.values()]
    __active.clear()
    for (const __timer of __timers) {
      __cancelTimer(__timer)
    }
  }
  __captureDisposer(__dispose)
  Reflect.apply(__register, __binding, [
    ...__registration,
    __schedule,
    __cancelTimerById,
  ])
  __assertManagedHostRegistrationActive(
    __binding,
    __isActive,
    __registration,
    'timer',
  )
}

function __assertManagedBindingUsable(__state) {
  if (__state.disposed) {
    throw new Error(__disposedBindingMessage)
  }
  if (__state.disposalStarted) {
    throw new Error(__disposingBindingMessage)
  }
}

function __createManagedBindingFacade(__binding, __state) {
  if (
    typeof WeakRef !== 'function' ||
    typeof FinalizationRegistry !== 'function'
  ) {
    throw new TypeError(
      'The managed workerd runtime requires WeakRef and FinalizationRegistry',
    )
  }
  const __bindingHolder = { value: __binding }
  const __bindingPrototypes = new Set()
  const __bindingConstructorPrototypes = new Set()
  const __constructorWrappers = new WeakMap()
  const __functionWrappers = new WeakMap()
  const __inputFunctionWrappers = new WeakMap()
  const __containerWrappers = new WeakMap()
  const __prototypeWrappers = new WeakMap()
  const __rawByWrapper = new WeakMap()
  const __objectWrappers = new WeakMap()
  const __bindingObjectTokens = new WeakMap()
  const __rawTargetHolderRefs = new Set()
  const __rawTargetHolderFinalizer = new FinalizationRegistry((__holderRef) => {
    __rawTargetHolderRefs.delete(__holderRef)
  })
  const __bindingObjectFinalizer = new FinalizationRegistry((__token) => {
    if (__token.released) return
    __token.released = true
    __state.openBindingObjects -= 1
  })
  const __rejectCloseReplacement = () => {
    throw new TypeError('Cannot replace or remove close on a managed workerd binding object')
  }
  const __defineManagedProperty = (
    __target,
    __property,
    __descriptor,
  ) => {
    if (__property !== 'close') {
      return Reflect.defineProperty(__target, __property, __descriptor)
    }
    const __current = Reflect.getOwnPropertyDescriptor(__target, __property)
    if (
      !__current ||
      ('value' in __descriptor &&
        (!('value' in __current) ||
          !Object.is(__descriptor.value, __current.value))) ||
      ('get' in __descriptor &&
        (!('get' in __current) ||
          !Object.is(__descriptor.get, __current.get))) ||
      ('set' in __descriptor &&
        (!('set' in __current) ||
          !Object.is(__descriptor.set, __current.set)))
    ) {
      __rejectCloseReplacement()
    }
    return Reflect.defineProperty(__target, __property, __descriptor)
  }

  const __unwrap = (__value) => __rawByWrapper.get(__value) ?? __value
  const __inputPassthroughPrototypes = new Set()
  const __rememberInputPassthroughPrototype = (__constructor) => {
    if (
      typeof __constructor === 'function' &&
      __constructor.prototype !== null &&
      typeof __constructor.prototype === 'object'
    ) {
      __inputPassthroughPrototypes.add(__constructor.prototype)
    }
  }
  for (const __name of [
    'ArrayBuffer',
    'SharedArrayBuffer',
    'DataView',
    'Date',
    'RegExp',
    'Map',
    'Set',
    'WeakMap',
    'WeakSet',
    'Promise',
    'WeakRef',
    'FinalizationRegistry',
    'Int8Array',
    'Uint8Array',
    'Uint8ClampedArray',
    'Int16Array',
    'Uint16Array',
    'Int32Array',
    'Uint32Array',
    'Float32Array',
    'Float64Array',
    'BigInt64Array',
    'BigUint64Array',
    'URL',
    'URLSearchParams',
    'Blob',
    'File',
    'FormData',
    'Headers',
    'Request',
    'Response',
    'ReadableStream',
    'WritableStream',
    'TransformStream',
  ]) {
    try {
      __rememberInputPassthroughPrototype(
        Reflect.get(globalThis, __name, globalThis),
      )
    } catch {}
  }
  try {
    __rememberInputPassthroughPrototype(Buffer)
  } catch {}
  try {
    const __webAssembly = Reflect.get(globalThis, 'WebAssembly', globalThis)
    for (const __name of ['Global', 'Memory', 'Module', 'Table']) {
      __rememberInputPassthroughPrototype(
        Reflect.get(__webAssembly, __name, __webAssembly),
      )
    }
  } catch {}
  const __isPlainContainer = (__value) => {
    if (__value === null || typeof __value !== 'object') return false
    if (Array.isArray(__value)) return true
    const __prototype = Reflect.getPrototypeOf(__value)
    return __prototype === Object.prototype || __prototype === null
  }
  const __assertUnseenPrototype = (__seen, __prototype) => {
    if (__seen.has(__prototype)) {
      throw new TypeError('Cyclic prototype chain detected in a managed workerd value')
    }
    if (__seen.size >= __prototypeTraversalLimit) {
      throw new TypeError('Managed workerd value prototype chain exceeds the traversal limit')
    }
    __seen.add(__prototype)
  }
  for (const __key of Reflect.ownKeys(__binding)) {
    const __descriptor = Reflect.getOwnPropertyDescriptor(__binding, __key)
    const __value = __descriptor && 'value' in __descriptor ? __descriptor.value : undefined
    if (
      typeof __value === 'function' &&
      __value.prototype !== null &&
      typeof __value.prototype === 'object'
    ) {
      const __prototype = __value.prototype
      __bindingPrototypes.add(__prototype)
      const __seen = new Set()
      let __constructorPrototype = __prototype
      while (
        __constructorPrototype !== null &&
        __constructorPrototype !== Object.prototype
      ) {
        __assertUnseenPrototype(__seen, __constructorPrototype)
        __bindingPrototypes.add(__constructorPrototype)
        __bindingConstructorPrototypes.add(__constructorPrototype)
        __constructorPrototype = Reflect.getPrototypeOf(
          __constructorPrototype,
        )
      }
    }
  }
  const __isInputRecord = (__value) => {
    if (__value === null || typeof __value !== 'object') return false
    if (
      ArrayBuffer.isView(__value) ||
      __hasIntrinsicArrayBufferBrand(__value)
    ) {
      return false
    }
    if (__isPlainContainer(__value)) return true
    const __seen = new Set()
    let __prototype
    try {
      __prototype = Reflect.getPrototypeOf(__value)
    } catch {
      return true
    }
    if (__inputPassthroughPrototypes.has(__prototype)) return false
    while (__prototype !== null) {
      __assertUnseenPrototype(__seen, __prototype)
      try {
        __prototype = Reflect.getPrototypeOf(__prototype)
      } catch {
        return true
      }
    }
    return true
  }
  const __findPropertyDescriptor = (__value, __key) => {
    const __seen = new Set()
    let __owner = __value
    while (__owner !== null) {
      __assertUnseenPrototype(__seen, __owner)
      const __descriptor = Reflect.getOwnPropertyDescriptor(__owner, __key)
      if (__descriptor) return __descriptor
      __owner = Reflect.getPrototypeOf(__owner)
    }
  }
  const __isCallablePropertyDescriptor = (__descriptor) =>
    __descriptor !== undefined &&
    (
      ('value' in __descriptor &&
        typeof __descriptor.value === 'function') ||
      (!('value' in __descriptor) &&
        typeof __descriptor.get === 'function')
    )
  const __isBindingObject = (__value) => {
    if ((__value === null || typeof __value !== 'object') && typeof __value !== 'function') {
      return false
    }
    let __prototype
    try {
      __prototype = Reflect.getPrototypeOf(__value)
    } catch {
      return false
    }
    const __seen = new Set()
    while (__prototype !== null) {
      __assertUnseenPrototype(__seen, __prototype)
      if (__bindingPrototypes.has(__prototype)) return true
      __prototype = Reflect.getPrototypeOf(__prototype)
    }
    return false
  }
  const __isConstructor = (__value) => {
    try {
      Reflect.construct(function () {}, [], __value)
      return true
    } catch {
      return false
    }
  }
  const __trackRawTargetHolder = (__holder, __wrapper) => {
    const __holderRef = new WeakRef(__holder)
    __rawTargetHolderRefs.add(__holderRef)
    __rawTargetHolderFinalizer.register(
      __wrapper,
      __holderRef,
      __holder,
    )
  }
  const __releaseBindingObject = (__raw) => {
    const __token = __bindingObjectTokens.get(__raw)
    if (!__token || __token.released) return
    __token.released = true
    __state.openBindingObjects -= 1
    __bindingObjectFinalizer.unregister(__token)
  }
  const __managedThenableCycleError = () =>
    new TypeError('Thenable cycle detected while settling a managed workerd call')
  const __hasCallableThenWithoutInvokingAccessor = (__value) => {
    const __descriptor = __findPropertyDescriptor(__value, 'then')
    if (!__descriptor) return false
    if ('value' in __descriptor) {
      return typeof __descriptor.value === 'function'
    }
    // This object already produced a callable then on the current path.
    // A surviving accessor is therefore the same cycle; deleting it still
    // permits mutable self-resolution without a second getter invocation.
    return typeof __descriptor.get === 'function'
  }
  const __boxManagedThenableResolution = (__thenableChain, __value) => {
    const __resolution = Object.create(null)
    __resolution.thenableChain = __thenableChain
    __resolution.value = __value
    return __resolution
  }
  const __invokeManagedThenable = (
    __value,
    __then,
    __onFulfilled,
    __onRejected,
  ) => {
    let __settled = false
    void Promise.resolve().then(() => {
      const __resolveOnce = (__resolved) => {
        if (__settled) return
        __settled = true
        try {
          __onFulfilled(__resolved)
        } catch (__error) {
          __onRejected(__error)
        }
      }
      const __rejectOnce = (__error) => {
        if (__settled) return
        __settled = true
        __onRejected(__error)
      }
      try {
        Reflect.apply(__then, __value, [__resolveOnce, __rejectOnce])
      } catch (__error) {
        __rejectOnce(__error)
      }
    })
  }
  const __resolveManagedThenable = (
    __value,
    __then,
    __thenableChain,
    __getPublicPromise,
  ) =>
    new Promise((__resolve, __reject) => {
      __invokeManagedThenable(__value, __then, (__resolved) => {
        if (__resolved === __getPublicPromise()) {
          throw __managedThenableCycleError()
        }
        if (
          (__resolved === null || typeof __resolved !== 'object') &&
          typeof __resolved !== 'function'
        ) {
          __resolve(__boxManagedThenableResolution(__thenableChain, __resolved))
          return
        }
        if (__thenableChain.has(__resolved)) {
          if (__hasCallableThenWithoutInvokingAccessor(__resolved)) {
            throw __managedThenableCycleError()
          }
          __resolve(__boxManagedThenableResolution(__thenableChain, __resolved))
          return
        }
        const __nextThen = Reflect.get(__resolved, 'then', __resolved)
        if (typeof __nextThen !== 'function') {
          __resolve(__boxManagedThenableResolution(__thenableChain, __resolved))
          return
        }
        const __nextThenableChain = new Set(__thenableChain)
        __nextThenableChain.add(__resolved)
        __resolveManagedThenable(
          __resolved,
          __nextThen,
          __nextThenableChain,
          __getPublicPromise,
        ).then(__resolve, __reject)
      }, __reject)
    })
  const __finishManagedCall = (
    __result,
    __mapResult,
    __onSuccess,
    __onFailure = () => {},
  ) => {
    let __then
    try {
      if (
        (__result !== null && typeof __result === 'object') ||
        typeof __result === 'function'
      ) {
        __then = Reflect.get(__result, 'then', __result)
      }
    } catch (__error) {
      __state.activeOperations -= 1
      return Promise.reject(__error)
    }
    if (typeof __then !== 'function') {
      try {
        __onSuccess()
        return __mapResult(__result)
      } catch (__error) {
        return Promise.reject(__error)
      } finally {
        __state.activeOperations -= 1
      }
    }

    let __publicPromise
    const __resolutionPromise = __resolveManagedThenable(
      __result,
      __then,
      new Set([__result]),
      () => __publicPromise,
    )
    __publicPromise = __resolutionPromise.then(
      (__resolution) => {
        try {
          __onSuccess()
          const __mapped = __mapResult(__resolution.value)
          return __mapped
        } finally {
          __state.activeOperations -= 1
        }
      },
      (__error) => {
        try {
          __onFailure()
        } catch {
        } finally {
          __state.activeOperations -= 1
        }
        throw __error
      },
    )
    return __publicPromise
  }
  const __finishInputCall = (__result) => {
    return __finishManagedCall(
      __result,
      __prepareBindingArgument,
      () => {},
    )
  }
  const __wrapInputFunction = (__fn, __owner) => {
    let __wrappers = __inputFunctionWrappers.get(__fn)
    if (!__wrappers) {
      __wrappers = new WeakMap()
      __inputFunctionWrappers.set(__fn, __wrappers)
    }
    const __ownerKey = __owner ?? __fn
    const __existing = __wrappers.get(__ownerKey)
    if (__existing) return __existing
    const __wrapper = function (...__args) {
      __assertManagedBindingUsable(__state)
      __state.activeOperations += 1
      let __result
      try {
        const __rawThis = __unwrap(this)
        __result = Reflect.apply(
          __fn,
          (__rawThis === undefined || __rawThis === null) &&
            __owner !== undefined
            ? __owner
            : __rawThis,
          __args.map(__wrapValue),
        )
      } catch (__error) {
        __state.activeOperations -= 1
        throw __error
      }
      return __finishInputCall(__result)
    }
    __wrappers.set(__ownerKey, __wrapper)
    return __wrapper
  }
  const __prepareBindingArgument = (
    __value,
    __seen = new WeakMap(),
    __owner,
  ) => {
    const __raw = __rawByWrapper.get(__value)
    if (__raw !== undefined) return __raw
    if (typeof __value === 'function') {
      return __wrapInputFunction(__value, __owner)
    }
    if (!__isInputRecord(__value)) return __value

    const __existing = __seen.get(__value)
    if (__existing) return __existing

    const __clone = Array.isArray(__value)
      ? []
      : Object.create(Reflect.getPrototypeOf(__value))
    __seen.set(__value, __clone)
    __rawByWrapper.set(__clone, __value)
    const __defined = new Set()
    const __descriptorPrototypes = new Set()
    let __descriptorOwner = __value
    while (
      __descriptorOwner !== null &&
      __descriptorOwner !== Object.prototype
    ) {
      __assertUnseenPrototype(
        __descriptorPrototypes,
        __descriptorOwner,
      )
      for (const __key of Reflect.ownKeys(__descriptorOwner)) {
        if (
          __defined.has(__key) ||
          __key === 'constructor' ||
          (Array.isArray(__value) && __key === 'length')
        ) {
          continue
        }
        const __descriptor = Reflect.getOwnPropertyDescriptor(
          __descriptorOwner,
          __key,
        )
        if (!__descriptor) continue
        __defined.add(__key)
        if ('value' in __descriptor) {
          __descriptor.value = __prepareBindingArgument(
            __descriptor.value,
            __seen,
            __value,
          )
        } else {
          const __get = __descriptor.get
          const __set = __descriptor.set
          __descriptor.get =
            typeof __get === 'function'
              ? function () {
                  return __prepareBindingArgument(
                    Reflect.apply(__get, __value, []),
                    new WeakMap(),
                    __value,
                  )
                }
              : undefined
          __descriptor.set =
            typeof __set === 'function'
              ? function (__nextValue) {
                  Reflect.apply(__set, __value, [
                    __prepareBindingArgument(
                      __nextValue,
                      new WeakMap(),
                      __value,
                    ),
                  ])
                }
              : undefined
        }
        Reflect.defineProperty(__clone, __key, __descriptor)
      }
      __descriptorOwner = Reflect.getPrototypeOf(__descriptorOwner)
    }
    if (Array.isArray(__value)) {
      Reflect.set(__clone, 'length', __value.length, __clone)
    }
    return __clone
  }
  const __prepareBindingArguments = (__values) => {
    const __seen = new WeakMap()
    return __values.map((__value) =>
      __prepareBindingArgument(__value, __seen),
    )
  }
  const __finishCall = (
    __result,
    __owner,
    __key,
    __target,
    __managedCloseOwner,
  ) => {
    const __closeOwner = __managedCloseOwner ?? __owner
    const __token = __bindingObjectTokens.get(__closeOwner)
    const __closesBindingObject =
      __key === 'close' &&
      __token !== undefined &&
      (
        __managedCloseOwner !== undefined ||
        __token.close === __target
      )
    return __finishManagedCall(
      __result,
      __wrapValue,
      () => {
        if (__closesBindingObject) __releaseBindingObject(__closeOwner)
      },
      () => {
        if (!__closesBindingObject) return
        try {
          if (Reflect.get(__closeOwner, 'closed', __closeOwner) === true) {
            __releaseBindingObject(__closeOwner)
          }
        } catch {}
      },
    )
  }
  const __wrapOutputDescriptor = (
    __descriptor,
    __owner,
    __key,
    __constructorWrapper,
  ) => {
    const __wrapped = { ...__descriptor }
    if ('value' in __wrapped) {
      if (__key === 'constructor' && __constructorWrapper) {
        __wrapped.value = __constructorWrapper
      } else {
        __wrapped.value =
          typeof __wrapped.value === 'function'
            ? __wrapFunction(__wrapped.value, __owner, __key)
            : __wrapValue(__wrapped.value)
      }
      return __wrapped
    }

    const __accessorHolder = {
      get: __wrapped.get,
      set: __wrapped.set,
      wrapper: undefined,
    }
    const __get =
      typeof __accessorHolder.get === 'function'
        ? function () {
            __assertManagedBindingUsable(__state)
            __state.activeOperations += 1
            try {
              const __rawThis = __unwrap(this)
              const __value = Reflect.apply(
                __accessorHolder.get,
                __rawThis,
                [],
              )
              if (typeof __value !== 'function') {
                return __wrapValue(__value)
              }
              const __functionOwner =
                (
                  (__rawThis !== null && typeof __rawThis === 'object') ||
                  typeof __rawThis === 'function'
                )
                  ? __rawThis
                  : __owner
              return __wrapFunction(
                __value,
                __functionOwner,
                __key,
                __key === 'close' &&
                  __bindingObjectTokens.has(__functionOwner)
                  ? __functionOwner
                  : undefined,
              )
            } finally {
              __state.activeOperations -= 1
            }
          }
        : undefined
    const __set =
      typeof __accessorHolder.set === 'function'
        ? function (__value) {
            __assertManagedBindingUsable(__state)
            __state.activeOperations += 1
            try {
              Reflect.apply(
                __accessorHolder.set,
                __unwrap(this),
                [__prepareBindingArgument(__value)],
              )
            } finally {
              __state.activeOperations -= 1
            }
          }
        : undefined
    __wrapped.get = __get
    __wrapped.set = __set
    __accessorHolder.wrapper = __get ?? __set
    if (__accessorHolder.wrapper) {
      __trackRawTargetHolder(
        __accessorHolder,
        __accessorHolder.wrapper,
      )
    }
    return __wrapped
  }
  const __wrapPrototype = (
    __rawPrototype,
    __constructorWrapper,
    __seen = new Set(),
  ) => {
    const __existing = __prototypeWrappers.get(__rawPrototype)
    if (__existing) return __existing
    __assertUnseenPrototype(__seen, __rawPrototype)

    const __rawParent = Reflect.getPrototypeOf(__rawPrototype)
    const __protectClosePath = __isCallablePropertyDescriptor(
      __findPropertyDescriptor(__rawPrototype, 'close'),
    )
    const __wrappedParent =
      __rawParent !== null && __rawParent !== Object.prototype
      ? __wrapPrototype(__rawParent, undefined, __seen)
      : __rawParent
    const __holder = {
      target: __rawPrototype,
      wrapper: undefined,
    }
    const __shadow = Object.create(__wrappedParent)
    const __wrapper = new Proxy(__shadow, {
      getPrototypeOf(__target) {
        __assertManagedBindingUsable(__state)
        return Reflect.getPrototypeOf(__target)
      },
      setPrototypeOf(__target, __prototype) {
        __assertManagedBindingUsable(__state)
        if (
          __protectClosePath &&
          !Object.is(Reflect.getPrototypeOf(__target), __prototype)
        ) {
          __rejectCloseReplacement()
        }
        return Reflect.setPrototypeOf(__target, __prototype)
      },
      has(__target, __property) {
        __assertManagedBindingUsable(__state)
        return Reflect.has(__target, __property)
      },
      ownKeys(__target) {
        __assertManagedBindingUsable(__state)
        return Reflect.ownKeys(__target)
      },
      getOwnPropertyDescriptor(__target, __property) {
        __assertManagedBindingUsable(__state)
        return Reflect.getOwnPropertyDescriptor(__target, __property)
      },
      isExtensible(__target) {
        __assertManagedBindingUsable(__state)
        return Reflect.isExtensible(__target)
      },
      preventExtensions(__target) {
        __assertManagedBindingUsable(__state)
        if (__protectClosePath && !Reflect.has(__target, 'close')) {
          __rejectCloseReplacement()
        }
        return Reflect.preventExtensions(__target)
      },
      get(__target, __property, __receiver) {
        __assertManagedBindingUsable(__state)
        return Reflect.get(__target, __property, __receiver)
      },
      set(__target, __property, __value, __receiver) {
        __assertManagedBindingUsable(__state)
        if (__property === 'close') __rejectCloseReplacement()
        return Reflect.set(__target, __property, __value, __receiver)
      },
      defineProperty(__target, __property, __descriptor) {
        __assertManagedBindingUsable(__state)
        return __defineManagedProperty(__target, __property, __descriptor)
      },
      deleteProperty(__target, __property) {
        __assertManagedBindingUsable(__state)
        if (__property === 'close') __rejectCloseReplacement()
        return Reflect.deleteProperty(__target, __property)
      },
    })
    __holder.wrapper = __wrapper
    __trackRawTargetHolder(__holder, __wrapper)
    __prototypeWrappers.set(__rawPrototype, __wrapper)
    __rawByWrapper.set(__wrapper, __rawPrototype)
    for (const __key of Reflect.ownKeys(__rawPrototype)) {
      const __descriptor = Reflect.getOwnPropertyDescriptor(__rawPrototype, __key)
      if (!__descriptor) continue
      const __rawConstructor =
        __key === 'constructor' && 'value' in __descriptor
          ? __descriptor.value
          : undefined
      Reflect.defineProperty(
        __shadow,
        __key,
        __wrapOutputDescriptor(
          __descriptor,
          __rawPrototype,
          __key,
          __constructorWrapper ??
            __constructorWrappers.get(__rawConstructor),
        ),
      )
    }
    return __wrapper
  }
  const __wrapFunction = (
    __fn,
    __owner,
    __key,
    __managedCloseOwner,
  ) => {
    const __existingConstructor = __constructorWrappers.get(__fn)
    if (__existingConstructor) return __existingConstructor

    let __wrappers = __functionWrappers.get(__owner)
    if (!__wrappers) {
      __wrappers = new Map()
      __functionWrappers.set(__owner, __wrappers)
    }
    const __existing = __wrappers.get(__key)
    if (__existing) {
      __existing.holder.target = __fn
      __existing.holder.managedCloseOwner = __managedCloseOwner
      __rawByWrapper.set(__existing.wrapper, __fn)
      return __existing.wrapper
    }

    const __holder = {
      key: __key,
      managedCloseOwner: __managedCloseOwner,
      owner: __owner,
      target: __fn,
      wrapper: undefined,
    }
    const __constructable = __isConstructor(__fn)
    const __constructorCloseDescriptor =
      __constructable &&
      __fn.prototype !== null &&
      typeof __fn.prototype === 'object'
        ? __findPropertyDescriptor(__fn.prototype, 'close')
        : undefined
    const __protectConstructorPrototype =
      __isCallablePropertyDescriptor(__constructorCloseDescriptor)
    const __shadow = __constructable ? function () {} : () => {}
    try {
      Reflect.defineProperty(__shadow, 'name', {
        configurable: true,
        value: __fn.name,
      })
      Reflect.defineProperty(__shadow, 'length', {
        configurable: true,
        value: __fn.length,
      })
    } catch {}
    const __wrapper = new Proxy(__shadow, {
      apply(_target, __thisArg, __args) {
        __assertManagedBindingUsable(__state)
        __state.activeOperations += 1
        let __result
        let __callOwner
        let __target
        try {
          __target = __holder.target
          const __rawThisArg = __unwrap(__thisArg)
          __result = Reflect.apply(
            __target,
            __rawThisArg,
            __prepareBindingArguments(__args),
          )
          __callOwner = __bindingObjectTokens.has(__rawThisArg)
            ? __rawThisArg
            : __holder.owner
        } catch (__error) {
          __state.activeOperations -= 1
          throw __error
        }
        return __finishCall(
          __result,
          __callOwner,
          __holder.key,
          __target,
          __holder.managedCloseOwner,
        )
      },
      construct(_target, __args, __newTarget) {
        __assertManagedBindingUsable(__state)
        __state.activeOperations += 1
        try {
          const __target = __holder.target
          const __unwrappedNewTarget = __rawByWrapper.get(__newTarget)
          const __exposedPrototype = Reflect.get(
            __newTarget,
            'prototype',
            __newTarget,
          )
          const __raw = Reflect.construct(
            __target,
            __prepareBindingArguments(__args),
            typeof __unwrappedNewTarget === 'function'
              ? __unwrappedNewTarget
              : __target,
          )
          return __wrapBindingObject(__raw, true, __exposedPrototype)
        } finally {
          __state.activeOperations -= 1
        }
      },
      get(__target, __property, __receiver) {
        __assertManagedBindingUsable(__state)
        return Reflect.get(__target, __property, __receiver)
      },
      set(__target, __property, __value, __receiver) {
        __assertManagedBindingUsable(__state)
        if (
          __protectConstructorPrototype &&
          __property === 'prototype' &&
          !Object.is(
            __value,
            Reflect.get(__target, __property, __target),
          )
        ) {
          __rejectCloseReplacement()
        }
        return Reflect.set(__target, __property, __value, __receiver)
      },
      defineProperty(__target, __property, __descriptor) {
        __assertManagedBindingUsable(__state)
        if (
          __protectConstructorPrototype &&
          __property === 'prototype'
        ) {
          const __current = Reflect.getOwnPropertyDescriptor(
            __target,
            __property,
          )
          if (
            !__current ||
            ('value' in __descriptor &&
              (!('value' in __current) ||
                !Object.is(__descriptor.value, __current.value))) ||
            'get' in __descriptor ||
            'set' in __descriptor
          ) {
            __rejectCloseReplacement()
          }
        }
        return Reflect.defineProperty(__target, __property, __descriptor)
      },
      deleteProperty(__target, __property) {
        __assertManagedBindingUsable(__state)
        if (
          __protectConstructorPrototype &&
          __property === 'prototype'
        ) {
          __rejectCloseReplacement()
        }
        return Reflect.deleteProperty(__target, __property)
      },
      has(__target, __property) {
        __assertManagedBindingUsable(__state)
        return Reflect.has(__target, __property)
      },
      ownKeys(__target) {
        __assertManagedBindingUsable(__state)
        return Reflect.ownKeys(__target)
      },
      getOwnPropertyDescriptor(__target, __property) {
        __assertManagedBindingUsable(__state)
        return Reflect.getOwnPropertyDescriptor(__target, __property)
      },
      getPrototypeOf(__target) {
        __assertManagedBindingUsable(__state)
        return Reflect.getPrototypeOf(__target)
      },
      setPrototypeOf(__target, __prototype) {
        __assertManagedBindingUsable(__state)
        return Reflect.setPrototypeOf(__target, __prototype)
      },
      isExtensible(__target) {
        __assertManagedBindingUsable(__state)
        return Reflect.isExtensible(__target)
      },
      preventExtensions(__target) {
        __assertManagedBindingUsable(__state)
        if (
          __protectConstructorPrototype &&
          !Reflect.has(
            Reflect.get(__target, 'prototype', __target),
            'close',
          )
        ) {
          __rejectCloseReplacement()
        }
        return Reflect.preventExtensions(__target)
      },
    })
    __holder.wrapper = __wrapper
    __trackRawTargetHolder(__holder, __wrapper)
    __wrappers.set(__key, { holder: __holder, wrapper: __wrapper })
    __rawByWrapper.set(__wrapper, __fn)
    const __bindingConstructor =
      __constructable &&
      __fn.prototype !== null &&
      typeof __fn.prototype === 'object' &&
      __bindingConstructorPrototypes.has(__fn.prototype)
    if (__bindingConstructor) {
      // Register before copying static properties so self and cross-constructor
      // references resolve to this canonical wrapper.
      __constructorWrappers.set(__fn, __wrapper)
    }
    for (const __property of Reflect.ownKeys(__fn)) {
      if (
        __property === 'name' ||
        __property === 'length' ||
        __property === 'arguments' ||
        __property === 'caller' ||
        (__property === 'prototype' && __constructable)
      ) {
        continue
      }
      const __descriptor = Reflect.getOwnPropertyDescriptor(__fn, __property)
      if (!__descriptor) continue
      Reflect.defineProperty(
        __shadow,
        __property,
        __wrapOutputDescriptor(__descriptor, __fn, __property, undefined),
      )
    }
    if (__bindingConstructor) {
      const __rawConstructorParent = Reflect.getPrototypeOf(__fn)
      const __wrappedConstructorParent =
        typeof __rawConstructorParent === 'function' &&
        __rawConstructorParent.prototype !== null &&
        typeof __rawConstructorParent.prototype === 'object' &&
        __bindingConstructorPrototypes.has(
          __rawConstructorParent.prototype,
        )
          ? __wrapFunction(
              __rawConstructorParent,
              __rawConstructorParent,
              __rawConstructorParent,
            )
          : __rawConstructorParent
      Reflect.setPrototypeOf(__shadow, __wrappedConstructorParent)
      const __prototypeDescriptor = Reflect.getOwnPropertyDescriptor(
        __fn,
        'prototype',
      )
      Reflect.defineProperty(
        __shadow,
        'prototype',
        {
          ...__prototypeDescriptor,
          value: __wrapPrototype(__fn.prototype, __wrapper),
        },
      )
    }
    return __wrapper
  }
  const __wrapBindingObject = (
    __raw,
    __force = false,
    __exposedPrototype,
  ) => {
    if (!__force && !__isBindingObject(__raw)) return __raw
    const __existing = __objectWrappers.get(__raw)
    if (__existing) return __existing

    const __rawPrototype = Reflect.getPrototypeOf(__raw)
    const __wrappedPrototype =
      __exposedPrototype ??
      (__bindingPrototypes.has(__rawPrototype)
        ? __wrapPrototype(__rawPrototype, undefined)
        : __rawPrototype)
    const __holder = {
      target: __raw,
      wrapper: undefined,
    }
    const __shadow = Object.create(__wrappedPrototype)
    const __suppressedRawProperties = new Set()
    const __rawShadowProperties = new Set()
    const __syncRawProperty = (__target, __key) => {
      if (__suppressedRawProperties.has(__key)) return
      const __descriptor = Reflect.getOwnPropertyDescriptor(
        __holder.target,
        __key,
      )
      const __current = Reflect.getOwnPropertyDescriptor(__target, __key)
      if (!__descriptor) {
        if (!__rawShadowProperties.has(__key)) return
        if (!__current || __current.configurable) {
          Reflect.deleteProperty(__target, __key)
          __rawShadowProperties.delete(__key)
        } else {
          throw new TypeError(
            'Raw binding state cannot be synchronized with the managed facade',
          )
        }
        return
      }
      if (
        !__rawShadowProperties.has(__key) &&
        __current
      ) {
        return
      }
      if (
        !__current &&
        !Reflect.isExtensible(__target)
      ) {
        return
      }
      const __wrappedDescriptor = __wrapOutputDescriptor(
        __descriptor,
        __holder.target,
        __key,
        undefined,
      )
      if (__current && !__current.configurable) {
        __wrappedDescriptor.configurable = false
        if ('value' in __current && 'value' in __wrappedDescriptor) {
          if (
            !__current.writable &&
            !Object.is(__current.value, __wrappedDescriptor.value)
          ) {
            throw new TypeError(
              'Raw binding state cannot be synchronized with the managed facade',
            )
          }
          if (!__current.writable) {
            __wrappedDescriptor.writable = false
          }
        } else if (
          !('value' in __current) &&
          !('value' in __wrappedDescriptor)
        ) {
          __wrappedDescriptor.get = __current.get
          __wrappedDescriptor.set = __current.set
        }
      }
      if (
        !Reflect.defineProperty(
          __target,
          __key,
          __wrappedDescriptor,
        )
      ) {
        throw new TypeError(
          'Raw binding state cannot be synchronized with the managed facade',
        )
      }
      __rawShadowProperties.add(__key)
    }
    const __syncRawProperties = (__target) => {
      for (const __key of Reflect.ownKeys(__holder.target)) {
        __syncRawProperty(__target, __key)
      }
      for (const __key of __rawShadowProperties) {
        __syncRawProperty(__target, __key)
      }
    }
    const __wrapper = new Proxy(__shadow, {
      getPrototypeOf(__target) {
        __assertManagedBindingUsable(__state)
        return Reflect.getPrototypeOf(__target)
      },
      setPrototypeOf(__target, __prototype) {
        __assertManagedBindingUsable(__state)
        if (
          __bindingObjectTokens.has(__holder.target) &&
          !Object.is(Reflect.getPrototypeOf(__target), __prototype)
        ) {
          __rejectCloseReplacement()
        }
        return Reflect.setPrototypeOf(__target, __prototype)
      },
      has(__target, __key) {
        __assertManagedBindingUsable(__state)
        __syncRawProperty(__target, __key)
        return (
          Reflect.has(__target, __key) ||
          (
            Reflect.isExtensible(__target) &&
            !__suppressedRawProperties.has(__key) &&
            Reflect.has(__holder.target, __key)
          )
        )
      },
      ownKeys(__target) {
        __assertManagedBindingUsable(__state)
        __syncRawProperties(__target)
        return Reflect.ownKeys(__target)
      },
      getOwnPropertyDescriptor(__target, __key) {
        __assertManagedBindingUsable(__state)
        __syncRawProperty(__target, __key)
        return Reflect.getOwnPropertyDescriptor(__target, __key)
      },
      isExtensible(__target) {
        __assertManagedBindingUsable(__state)
        return Reflect.isExtensible(__target)
      },
      preventExtensions(__target) {
        __assertManagedBindingUsable(__state)
        if (
          __bindingObjectTokens.has(__holder.target) &&
          !Reflect.has(__target, 'close')
        ) {
          __rejectCloseReplacement()
        }
        __syncRawProperties(__target)
        return Reflect.preventExtensions(__target)
      },
      get(__target, __key, __receiver) {
        __assertManagedBindingUsable(__state)
        __state.activeOperations += 1
        try {
          __syncRawProperty(__target, __key)
          if (Reflect.has(__target, __key)) {
            return Reflect.get(__target, __key, __receiver)
          }
          if (
            __suppressedRawProperties.has(__key) ||
            !Reflect.isExtensible(__target)
          ) {
            return undefined
          }
          const __rawTarget = __holder.target
          const __value = Reflect.get(__rawTarget, __key, __rawTarget)
          return typeof __value === 'function'
            ? __wrapFunction(
                __value,
                __rawTarget,
                __key,
                __key === 'close' &&
                  __bindingObjectTokens.has(__rawTarget)
                  ? __rawTarget
                  : undefined,
              )
            : __wrapValue(__value)
        } finally {
          __state.activeOperations -= 1
        }
      },
      set(__target, __key, __value, __receiver) {
        __assertManagedBindingUsable(__state)
        __state.activeOperations += 1
        try {
          if (
            __key === 'close' &&
            __bindingObjectTokens.has(__holder.target)
          ) {
            __rejectCloseReplacement()
          }
          const __updated = Reflect.set(
            __target,
            __key,
            __value,
            __receiver,
          )
          if (__updated) {
            __suppressedRawProperties.delete(__key)
            __rawShadowProperties.delete(__key)
          }
          return __updated
        } finally {
          __state.activeOperations -= 1
        }
      },
      defineProperty(__target, __key, __descriptor) {
        __assertManagedBindingUsable(__state)
        if (
          __rawShadowProperties.has(__key) &&
          !('value' in __descriptor) &&
          !('get' in __descriptor) &&
          !('set' in __descriptor)
        ) {
          const __rawDescriptor = Reflect.getOwnPropertyDescriptor(
            __holder.target,
            __key,
          )
          if (__rawDescriptor) {
            const __rawIntegrityDescriptor = {}
            if ('configurable' in __descriptor) {
              __rawIntegrityDescriptor.configurable =
                __descriptor.configurable
            }
            if ('enumerable' in __descriptor) {
              __rawIntegrityDescriptor.enumerable =
                __descriptor.enumerable
            }
            if (
              'writable' in __descriptor &&
              'value' in __rawDescriptor
            ) {
              __rawIntegrityDescriptor.writable =
                __descriptor.writable
            }
            if (
              !Reflect.defineProperty(
                __holder.target,
                __key,
                __rawIntegrityDescriptor,
              )
            ) {
              return false
            }
          }
          return __defineManagedProperty(
            __target,
            __key,
            __descriptor,
          )
        }
        const __defined = __defineManagedProperty(
          __target,
          __key,
          __descriptor,
        )
        if (__defined) {
          __suppressedRawProperties.delete(__key)
          __rawShadowProperties.delete(__key)
        }
        return __defined
      },
      deleteProperty(__target, __key) {
        __assertManagedBindingUsable(__state)
        if (__key === 'close') __rejectCloseReplacement()
        const __deleted = Reflect.deleteProperty(__target, __key)
        if (__deleted && Reflect.has(__holder.target, __key)) {
          __suppressedRawProperties.add(__key)
        }
        return __deleted
      },
    })
    __holder.wrapper = __wrapper
    __trackRawTargetHolder(__holder, __wrapper)
    __objectWrappers.set(__raw, __wrapper)
    __rawByWrapper.set(__wrapper, __raw)
    __syncRawProperties(__shadow)

    const __close = Reflect.get(__raw, 'close', __raw)
    if (typeof __close === 'function') {
      const __token = { close: __close, released: false }
      __bindingObjectTokens.set(__raw, __token)
      __state.openBindingObjects += 1
      __bindingObjectFinalizer.register(__wrapper, __token, __token)
    }
    return __wrapper
  }
  const __wrapContainer = (__raw) => {
    const __existing = __containerWrappers.get(__raw)
    if (__existing) return __existing

    const __clone = Array.isArray(__raw)
      ? []
      : Object.create(Reflect.getPrototypeOf(__raw))
    __containerWrappers.set(__raw, __clone)
    for (const __key of Reflect.ownKeys(__raw)) {
      if (Array.isArray(__raw) && __key === 'length') continue
      const __descriptor = Reflect.getOwnPropertyDescriptor(__raw, __key)
      if (!__descriptor) continue
      if ('value' in __descriptor) {
        __descriptor.value =
          typeof __descriptor.value === 'function'
            ? __wrapFunction(__descriptor.value, __raw, __key)
            : __wrapValue(__descriptor.value)
      } else {
        const __accessorHolder = {
          get: __descriptor.get,
          set: __descriptor.set,
          target: __raw,
          wrapper: undefined,
        }
        const __get =
          typeof __accessorHolder.get === 'function'
            ? function () {
                __assertManagedBindingUsable(__state)
                const __rawTarget = __accessorHolder.target
                const __value = Reflect.apply(
                  __accessorHolder.get,
                  __rawTarget,
                  [],
                )
                return typeof __value === 'function'
                  ? __wrapFunction(__value, __rawTarget, __key)
                  : __wrapValue(__value)
              }
            : undefined
        const __set =
          typeof __accessorHolder.set === 'function'
            ? function (__value) {
                __assertManagedBindingUsable(__state)
                const __rawTarget = __accessorHolder.target
                Reflect.apply(
                  __accessorHolder.set,
                  __rawTarget,
                  [__prepareBindingArgument(__value)],
                )
              }
            : undefined
        __descriptor.get = __get
        __descriptor.set = __set
        __accessorHolder.wrapper = __get ?? __set
        if (__accessorHolder.wrapper) {
          __trackRawTargetHolder(
            __accessorHolder,
            __accessorHolder.wrapper,
          )
        }
      }
      Reflect.defineProperty(__clone, __key, __descriptor)
    }
    if (Array.isArray(__raw)) {
      Reflect.set(__clone, 'length', __raw.length, __clone)
    }
    return __clone
  }
  const __wrapValue = (__value) => {
    if (__value === __bindingHolder.value) return __facade
    if (typeof __value === 'function') {
      const __constructorWrapper = __constructorWrappers.get(__value)
      if (__constructorWrapper) return __constructorWrapper
      return __wrapFunction(__value, __value, __value)
    }
    if (__isBindingObject(__value)) return __wrapBindingObject(__value)
    if (__isPlainContainer(__value)) return __wrapContainer(__value)
    return __value
  }

  const __facade = {}
  for (const __key of Reflect.ownKeys(__bindingHolder.value)) {
    if (__managedPrivateBindingExports.has(__key)) continue
    const __descriptor = Reflect.getOwnPropertyDescriptor(__bindingHolder.value, __key)
    if (!__descriptor) continue
    if ('value' in __descriptor) {
      const __value =
        typeof __descriptor.value === 'function'
          ? __wrapFunction(__descriptor.value, __bindingHolder.value, __key)
          : __wrapValue(__descriptor.value)
      Reflect.defineProperty(__facade, __key, {
        configurable: false,
        enumerable: __descriptor.enumerable,
        value: __value,
        writable: false,
      })
    } else {
      Reflect.defineProperty(__facade, __key, {
        configurable: false,
        enumerable: __descriptor.enumerable,
        get() {
          __assertManagedBindingUsable(__state)
          const __value = Reflect.get(
            __bindingHolder.value,
            __key,
            __bindingHolder.value,
          )
          return typeof __value === 'function'
            ? __wrapFunction(
                __value,
                __bindingHolder.value,
                __key,
              )
            : __wrapValue(__value)
        },
      })
    }
  }
  __state.releaseBindingFacade = () => {
    for (const __holderRef of __rawTargetHolderRefs) {
      const __holder = __holderRef.deref()
      if (!__holder) continue
      __rawTargetHolderFinalizer.unregister(__holder)
      __rawByWrapper.delete(__holder.wrapper)
      __holder.get = undefined
      __holder.owner = undefined
      __holder.set = undefined
      __holder.target = undefined
      __holder.wrapper = undefined
    }
    __rawTargetHolderRefs.clear()
    __bindingPrototypes.clear()
    __bindingConstructorPrototypes.clear()
    __bindingHolder.value = undefined
    __state.releaseBindingFacade = undefined
  }
  return Object.freeze(__facade)
}

function __attachCleanupError(__error, __cleanupError) {
  return new AggregateError(
    [__error, __cleanupError],
    'Managed workerd initialization failed and context cleanup did not complete',
    { cause: __error },
  )
}

function __retryContextSetupOperation(__operation, __message) {
  const __errors = []
  for (let __attempt = 0; __attempt < 2; __attempt += 1) {
    try {
      return __operation()
    } catch (__error) {
      __errors.push(__error)
    }
  }
  throw new AggregateError(__errors, __message)
}

function __collectContextDestroyErrors(__context) {
  const __cleanupErrors = []
  for (let __attempt = 0; __attempt < 2; __attempt += 1) {
    try {
      __context.destroy()
      return []
    } catch (__cleanupError) {
      __cleanupErrors.push(__cleanupError)
    }
  }
  return __cleanupErrors
}

function __destroyContextAfterFailure(__context, __error) {
  const __cleanupErrors = __collectContextDestroyErrors(__context)
  if (__cleanupErrors.length === 0) return __error
  return __attachCleanupError(
    __error,
    new AggregateError(__cleanupErrors, 'Managed workerd context cleanup failed'),
  )
}

function __createManagedContext() {
  const __nodeProcess = globalThis.process
  const __beforeExitListeners =
    typeof __nodeProcess?.rawListeners === 'function'
      ? __nodeProcess.rawListeners('beforeExit')
      : undefined
  const __newListeners =
    typeof __nodeProcess?.rawListeners === 'function'
      ? __nodeProcess.rawListeners('newListener')
      : undefined
  const __maxListeners =
    __beforeExitListeners &&
    __newListeners &&
    typeof __nodeProcess.getMaxListeners === 'function' &&
    typeof __nodeProcess.setMaxListeners === 'function'
      ? __nodeProcess.getMaxListeners()
      : undefined
  let __maxListenersRaised = false
  if (
    __maxListeners &&
    __maxListeners > 0 &&
    Math.max(__beforeExitListeners.length, __newListeners.length) >= __maxListeners
  ) {
    try {
      __nodeProcess.setMaxListeners(__maxListeners + 1)
      __maxListenersRaised = true
    } catch (__error) {
      try {
        __retryContextSetupOperation(
          () => __nodeProcess.setMaxListeners(__maxListeners),
          'Managed workerd listener-limit rollback failed',
        )
      } catch (__cleanupError) {
        throw __attachCleanupError(__error, __cleanupError)
      }
      throw __error
    }
  }

  let __emnapiContext
  let __captureListener
  let __autoDestroyListener
  let __captureListenerInstalled = false
  let __setupFailed = false
  let __setupError
  try {
    if (
      typeof __nodeProcess?.prependListener === 'function' &&
      typeof __nodeProcess.removeListener === 'function'
    ) {
      __captureListener = (__event, __listener) => {
        if (__event === 'beforeExit' && __autoDestroyListener === undefined) {
          __autoDestroyListener = __listener
        }
      }
      __retryContextSetupOperation(
        () => __nodeProcess.prependListener('newListener', __captureListener),
        'Managed workerd newListener capture registration failed',
      )
      __captureListenerInstalled = true
    }
    __emnapiContext = __emnapiCreateContext({ autoDestroy: false })
    __emnapiContext.feature.Buffer = Buffer
    __emnapiContext.suppressDestroy()
  } catch (__error) {
    __setupFailed = true
    __setupError = __error
  }

  const __setupCleanupErrors = []
  if (__captureListenerInstalled) {
    try {
      __retryContextSetupOperation(
        () => __nodeProcess.removeListener('newListener', __captureListener),
        'Managed workerd newListener capture cleanup failed',
      )
    } catch (__error) {
      __setupCleanupErrors.push(__error)
    }
  }
  // emnapi <= 1.11 ignores autoDestroy. Remove only the exact listener it
  // registered; suppressDestroy() remains the safety net if capture is absent.
  if (
    __autoDestroyListener !== undefined &&
    typeof __nodeProcess?.removeListener === 'function'
  ) {
    try {
      __retryContextSetupOperation(
        () => __nodeProcess.removeListener('beforeExit', __autoDestroyListener),
        'Managed workerd beforeExit listener cleanup failed',
      )
    } catch (__error) {
      __setupCleanupErrors.push(__error)
    }
  }
  if (__maxListenersRaised) {
    try {
      __retryContextSetupOperation(
        () => __nodeProcess.setMaxListeners(__maxListeners),
        'Managed workerd listener-limit restoration failed',
      )
    } catch (__error) {
      __setupCleanupErrors.push(__error)
    }
  }

  if (__setupFailed || __setupCleanupErrors.length > 0) {
    if (__emnapiContext) {
      const __destroyErrors = []
      let __destroyed = false
      for (let __attempt = 0; __attempt < 2; __attempt += 1) {
        try {
          __emnapiContext.destroy()
          __destroyed = true
          break
        } catch (__error) {
          __destroyErrors.push(__error)
        }
      }
      if (!__destroyed) {
        __setupCleanupErrors.push(
          new AggregateError(__destroyErrors, 'Managed workerd context setup cleanup failed'),
        )
      }
    }
    if (__setupFailed) {
      if (__setupCleanupErrors.length > 0) {
        throw __attachCleanupError(
          __setupError,
          new AggregateError(
            __setupCleanupErrors,
            'Managed workerd context setup cleanup failed',
          ),
        )
      }
      throw __setupError
    }
    if (__setupCleanupErrors.length === 1) {
      throw __setupCleanupErrors[0]
    }
    throw new AggregateError(
      __setupCleanupErrors,
      'Managed workerd context setup cleanup failed',
    )
  }

  return __emnapiContext
}

async function __instantiate(
  __wasmInput,
  __options = {},
  __emnapiContext = __emnapiGetDefaultContext(),
  __claimMemory = false,
) {
  const __module = await __wasmInput
  __validateModule(__module)

  if (
    __options.memory &&
    (__options.initialMemoryPages !== undefined || __options.maximumMemoryPages !== undefined)
  ) {
    throw new TypeError(
      'Pass either memory or initialMemoryPages/maximumMemoryPages, not both',
    )
  }

  const __wasmMemory =
    __options.memory ??
    new WebAssembly.Memory({
      initial: __options.initialMemoryPages ?? WORKERD_WASM_MEMORY.initialPages,
      maximum: __options.maximumMemoryPages ?? WORKERD_WASM_MEMORY.maximumPages,
    })
  __validateMemory(__wasmMemory)
  // A failed instantiation may already have mutated memory before throwing.
  // Claim before entering emnapi and never make caller memory reusable.
  if (__claimMemory) __claimManagedMemoryForAttempt(__wasmMemory)

  const __wasi = new __WASI({ version: 'preview1' })
  const { napiModule: __napiModule } = await __emnapiInstantiateNapiModule(__module, {
    context: __emnapiContext,
    asyncWorkPoolSize: 0,
    wasi: __wasi,
    overwriteImports(importObject) {
      importObject.env = {
        ...importObject.env,
        ...importObject.napi,
        ...importObject.emnapi,
        memory: __wasmMemory,
      }
      return importObject
    },
    beforeInit({ instance }) {
      for (const name of Object.keys(instance.exports)) {
        if (name.startsWith('__napi_register__')) {
          instance.exports[name]()
        }
      }
    },
  })

  return { exports: __napiModule.exports, memory: __wasmMemory }
}

/**
 * Create one independent N-API/WASI instance. Concurrent calls never share
 * memory or runtime state. A caller-provided memory is consumed by the first
 * validated initialization attempt, including one that later fails. Dispose
 * only after closing every build created by this binding; disposal runs
 * cleanup hooks, releases references, and is idempotent.
 */
export async function createInstance(__wasmInput, __options) {
  const __module = await __wasmInput
  __validateModule(__module)
  let __emnapiContext = __createManagedContext()

  let __exports
  let __memory
  try {
    const __instance = await __instantiate(__module, __options, __emnapiContext, true)
    __exports = __instance.exports
    __memory = __instance.memory
  } catch (__error) {
    // No handle can be returned for a later cleanup retry. Retry once now,
    // while preserving initialization as the primary failure.
    throw __destroyContextAfterFailure(__emnapiContext, __error)
  }

  let __disposeTaskHost
  let __disposeTimerHost
  const __disposeHosts = (__attempts = 1) => {
    const __cleanupErrors = []
    const __disposeHost = (__getDisposer, __clearDisposer) => {
      const __hostErrors = []
      for (let __attempt = 0; __attempt < __attempts; __attempt += 1) {
        const __disposer = __getDisposer()
        if (!__disposer) return
        try {
          __disposer()
          __clearDisposer()
          return
        } catch (__error) {
          __hostErrors.push(__error)
        }
      }
      if (__getDisposer()) __cleanupErrors.push(...__hostErrors)
    }
    __disposeHost(
      () => __disposeTimerHost,
      () => {
        __disposeTimerHost = undefined
      },
    )
    __disposeHost(
      () => __disposeTaskHost,
      () => {
        __disposeTaskHost = undefined
      },
    )
    return __cleanupErrors
  }
  const __throwHostCleanupErrors = (__cleanupErrors) => {
    if (__cleanupErrors.length === 1) throw __cleanupErrors[0]
    if (__cleanupErrors.length > 1) {
      throw new AggregateError(__cleanupErrors, 'Managed workerd host cleanup failed')
    }
  }
  const __state = {
    activeOperations: 0,
    disposalStarted: false,
    disposed: false,
    disposing: false,
    openBindingObjects: 0,
  }
  let __publicExports
  try {
    __registerManagedCurrentThreadTaskHost(__exports, (__dispose) => {
      __disposeTaskHost = __dispose
    })
    __registerManagedTimerHost(__exports, (__dispose) => {
      __disposeTimerHost = __dispose
    })
    for (const __privateExport of __managedPrivateBindingExports) {
      if (
        Reflect.has(__exports, __privateExport) &&
        !Reflect.deleteProperty(__exports, __privateExport)
      ) {
        throw new TypeError(
          'Unable to hide the private ' + __privateExport + ' host control',
        )
      }
      if (Reflect.has(__exports, __privateExport)) {
        throw new TypeError(
          'The private ' + __privateExport + ' host control remains reachable',
        )
      }
    }
    __publicExports = __createManagedBindingFacade(__exports, __state)
  } catch (__error) {
    const __cleanupErrors = __disposeHosts(2)
    const __contextErrors = __collectContextDestroyErrors(__emnapiContext)
    if (__contextErrors.length > 0) {
      __cleanupErrors.push(
        new AggregateError(
          __contextErrors,
          'Managed workerd context cleanup failed',
        ),
      )
    }
    if (__cleanupErrors.length > 0) {
      throw __attachCleanupError(
        __error,
        new AggregateError(
          __cleanupErrors,
          'Managed workerd initialization cleanup failed',
        ),
      )
    }
    throw __error
  }

  __createdManagedInstances += 1
  __liveManagedInstances += 1

  return Object.freeze({
    get exports() {
      __assertManagedBindingUsable(__state)
      return __publicExports
    },
    get memory() {
      if (__state.disposed) throw new Error(__disposedBindingMessage)
      return __memory
    },
    get memoryBytes() {
      if (__state.disposed) return 0
      return __memory.buffer.byteLength
    },
    get disposed() {
      return __state.disposed
    },
    dispose() {
      if (__state.disposing) return
      if (__state.disposed) {
        __throwHostCleanupErrors(__disposeHosts())
        return
      }
      if (!__state.disposalStarted) {
        const __blockers = []
        if (__state.activeOperations > 0) {
          __blockers.push(
            \`\${__state.activeOperations} active binding operation\${
              __state.activeOperations === 1 ? '' : 's'
            }\`,
          )
        }
        if (__state.openBindingObjects > 0) {
          __blockers.push(
            \`\${__state.openBindingObjects} open binding object\${
              __state.openBindingObjects === 1 ? '' : 's'
            }\`,
          )
        }
        if (__blockers.length > 0) {
          throw new Error(
            \`Cannot dispose this workerd Rolldown instance with \${__blockers.join(
              ' and ',
            )}; await active operations and close every binding object first\`,
          )
        }
        __state.disposalStarted = true
      }
      const __context = __emnapiContext
      // Keep the handle and context intact if cleanup throws so the caller can
      // retry disposal. Marking it disposed first would permanently leak a
      // partially destroyed N-API environment.
      __state.disposing = true
      try {
        // Explicitly evict the exact Rust registrations while the environment
        // is still callable. emnapi stops at the first throwing cleanup hook,
        // so relying on hook order can otherwise leave a disabled host selected.
        __throwHostCleanupErrors(__disposeHosts())
        __context.destroy()
        __state.disposed = true
        __liveManagedInstances -= 1
        __state.releaseBindingFacade()
        __emnapiContext = undefined
        __exports = undefined
        __publicExports = undefined
        __memory = undefined
        __throwHostCleanupErrors(__disposeHosts())
      } finally {
        __state.disposing = false
      }
    },
  })
}

/** Compatibility alias for the managed factory. */
export const instantiate = createInstance
`;
}

function renderWorkerdLoaderDeclaration(): string {
  return `import type * as RolldownBinding from './rolldown-binding.wasip1.cjs';

export interface DeferredInstanceOptions {
  /**
   * Caller-provided memory is single-use for managed initialization attempts.
   * Once validation succeeds and initialization begins, it cannot be passed
   * to another createInstance call, even if initialization fails or the
   * created instance is later disposed.
   */
  memory?: WebAssembly.Memory;
  initialMemoryPages?: number;
  maximumMemoryPages?: number;
}

export interface DeferredRuntimeStats {
  /** Counts instances created by this evaluated loader module, not process-wide instances. */
  createdInstances: number;
  /** Counts live instances created by this evaluated loader module. */
  liveInstances: number;
  declaredInitialMemoryBytes: number;
}

export type DeferredRolldownBinding = Omit<
  typeof RolldownBinding,
  | 'getCurrentThreadTaskHostContractVersion'
  | 'isCurrentThreadHostRegistrationActive'
  | 'registerCurrentThreadTaskHost'
  | 'registerTimerHost'
  | 'reserveCurrentThreadHostRegistration'
  | 'unregisterCurrentThreadTaskHost'
  | 'unregisterTimerHost'
>;

export interface DeferredRolldownInstance {
  readonly exports: DeferredRolldownBinding;
  readonly memory: WebAssembly.Memory;
  readonly memoryBytes: number;
  readonly disposed: boolean;
  /**
   * Destroy this instance's N-API environment. Active binding operations and
   * unclosed binding objects reject disposal before cleanup begins. If cleanup
   * throws, the handle remains undisposed and a later call retries cleanup.
   */
  dispose(): void;
}

export const WORKERD_WASM_MEMORY: Readonly<{
  initialPages: number;
  maximumPages: number;
  pageBytes: number;
  initialBytes: number;
  maximumBytes: number;
}>;

export function getDeferredRuntimeStats(): Readonly<DeferredRuntimeStats>;

export function createInstance(
  module: WebAssembly.Module | PromiseLike<WebAssembly.Module>,
  options?: DeferredInstanceOptions,
): Promise<DeferredRolldownInstance>;

export const instantiate: typeof createInstance;
`;
}

function renderCurrentThreadHostBootstrap(captureBrowserRegistrations = false): string {
  const captureTaskHostRegistration = captureBrowserRegistrations
    ? '  __browserTaskHostRegistration = __taskHostRegistration'
    : '  __nodeTaskHostRegistration = __taskHostRegistration';
  const captureTimerHostRegistration = captureBrowserRegistrations
    ? '    __browserTimerHostRegistration = __timerHostRegistration'
    : '    __nodeTimerHostRegistration = __timerHostRegistration';
  return `${CURRENT_THREAD_HOST_BOOTSTRAP_START}
;{
  const __rolldownBinding = __napiModule.exports
  const __getCurrentThreadTaskHostContractVersion =
    __rolldownBinding.getCurrentThreadTaskHostContractVersion
  const __isCurrentThreadHostRegistrationActive =
    __rolldownBinding.isCurrentThreadHostRegistrationActive
  const __registerCurrentThreadTaskHost =
    __rolldownBinding.registerCurrentThreadTaskHost
  const __registerTimerHost = __rolldownBinding.registerTimerHost
  const __reserveCurrentThreadHostRegistration =
    __rolldownBinding.reserveCurrentThreadHostRegistration
  const __unregisterCurrentThreadTaskHost =
    __rolldownBinding.unregisterCurrentThreadTaskHost
  const __unregisterTimerHost = __rolldownBinding.unregisterTimerHost
  if (
    typeof __getCurrentThreadTaskHostContractVersion !== 'function' ||
    typeof __isCurrentThreadHostRegistrationActive !== 'function' ||
    typeof __registerCurrentThreadTaskHost !== 'function' ||
    typeof __registerTimerHost !== 'function' ||
    typeof __reserveCurrentThreadHostRegistration !== 'function' ||
    typeof __unregisterCurrentThreadTaskHost !== 'function' ||
    typeof __unregisterTimerHost !== 'function'
  ) {
    throw new TypeError(
      'The threadless Rolldown binding does not expose its CurrentThread host integration',
    )
  }
  const __taskHostContractVersion =
    Reflect.apply(
      __getCurrentThreadTaskHostContractVersion,
      __rolldownBinding,
      [],
    )
  if (__taskHostContractVersion !== ${CURRENT_THREAD_TASK_HOST_CONTRACT_VERSION}) {
    throw new TypeError(
      'The threadless Rolldown binding uses CurrentThread task-host contract version ' +
        String(__taskHostContractVersion) +
        ', but version ${CURRENT_THREAD_TASK_HOST_CONTRACT_VERSION} is required',
    )
  }
  const __readHostRegistration = (__registration, __label) => {
    let __high
    let __low
    try {
      __high = Reflect.get(__registration, 'high', __registration)
      __low = Reflect.get(__registration, 'low', __registration)
    } catch {}
    if (
      !Number.isInteger(__high) ||
      __high < 0 ||
      __high > 0xffffffff ||
      !Number.isInteger(__low) ||
      __low < 0 ||
      __low > 0xffffffff ||
      (__high === 0 && __low === 0)
    ) {
      throw new TypeError(
        'The threadless Rolldown binding returned an invalid ' +
          __label +
          ' host registration',
      )
    }
    return { high: __high, low: __low }
  }
  const __assertHostRegistrationActive = (__registration, __label) => {
    const __active = Reflect.apply(
      __isCurrentThreadHostRegistrationActive,
      __rolldownBinding,
      [__registration.high, __registration.low],
    )
    if (typeof __active !== 'boolean') {
      throw new TypeError(
        'The threadless Rolldown binding returned an invalid ' +
          __label +
          ' host liveness result',
      )
    }
    if (!__active) {
      throw new TypeError(
        'The threadless Rolldown binding returned an inactive ' +
          __label +
          ' host registration',
      )
    }
  }
  const __taskHostRegistration = __readHostRegistration(
    Reflect.apply(
      __reserveCurrentThreadHostRegistration,
      __rolldownBinding,
      [],
    ),
    'task',
  )
${captureTaskHostRegistration}
  Reflect.apply(__registerCurrentThreadTaskHost, __rolldownBinding, [
    __taskHostRegistration.high,
    __taskHostRegistration.low,
  ])
  __assertHostRegistrationActive(__taskHostRegistration, 'task')

  const __setTimeoutHost = globalThis.setTimeout?.bind(globalThis)
  const __clearTimeoutHost = globalThis.clearTimeout?.bind(globalThis)
  if (__setTimeoutHost && __clearTimeoutHost) {
    const __MAX_HOST_TIMEOUT_MS = 2147483647
    const __activeTimers = new Map()
    const __armTimer = (__id, __timer) => {
      const __delay = Math.min(__timer.remainingMs, __MAX_HOST_TIMEOUT_MS)
      __timer.handle = __setTimeoutHost(() => {
        if (__activeTimers.get(__id) !== __timer) return
        __timer.remainingMs -= __delay
        if (__timer.remainingMs > 0) {
          try {
            __armTimer(__id, __timer)
          } catch (__error) {
            __activeTimers.delete(__id)
            __timer.reject(__error)
          }
          return
        }
        __activeTimers.delete(__id)
        __timer.resolve()
      }, __delay)
    }
    const __cancelTimer = (__timer) => {
      try {
        if (__timer.handle !== undefined) {
          __clearTimeoutHost(__timer.handle)
        }
      } catch {
        // Rust invokes this callback through a non-catching TSFN. Contain
        // host cancellation failures at the JavaScript boundary.
      } finally {
        __timer.resolve()
      }
    }
    const __timerHostRegistration = __readHostRegistration(
      Reflect.apply(
        __reserveCurrentThreadHostRegistration,
        __rolldownBinding,
        [],
      ),
      'timer',
    )
${captureTimerHostRegistration}
    Reflect.apply(__registerTimerHost, __rolldownBinding, [
      __timerHostRegistration.high,
      __timerHostRegistration.low,
      (__id, __ms) => {
        const __previous = __activeTimers.get(__id)
        if (__previous) {
            __activeTimers.delete(__id)
            __cancelTimer(__previous)
          }
          return new Promise((__resolve, __reject) => {
            const __timer = {
              handle: undefined,
              remainingMs: Math.max(__ms, 0),
              reject: __reject,
              resolve: __resolve,
            }
            __activeTimers.set(__id, __timer)
            try {
              __armTimer(__id, __timer)
            } catch (__error) {
              if (__activeTimers.get(__id) === __timer) {
                __activeTimers.delete(__id)
              }
              __reject(__error)
            }
          })
        },
        (__id) => {
          const __timer = __activeTimers.get(__id)
          if (!__timer) return
          __activeTimers.delete(__id)
          __cancelTimer(__timer)
        },
      ])
    __assertHostRegistrationActive(__timerHostRegistration, 'timer')
  }
}
${CURRENT_THREAD_HOST_BOOTSTRAP_END}`;
}

function injectBrowserInitializationGuard(
  source: string,
  loaderPath: string,
  exportAnchor: string,
): string {
  const guardStartCount = source.split(BROWSER_INITIALIZATION_GUARD_START).length - 1;
  const guardEndCount = source.split(BROWSER_INITIALIZATION_GUARD_END).length - 1;
  if (guardStartCount !== guardEndCount || guardStartCount > 1) {
    throw new Error(`Malformed browser initialization guard in ${loaderPath}`);
  }
  const exportCount = source.split(exportAnchor).length - 1;
  if (exportCount !== 1) {
    throw new Error(`Unable to locate generated browser export anchor in ${loaderPath}`);
  }
  const exportStart = source.indexOf(exportAnchor);
  if (guardStartCount === 1) {
    const guardStart = source.indexOf(BROWSER_INITIALIZATION_GUARD_START);
    const guardEnd = source.indexOf(BROWSER_INITIALIZATION_GUARD_END, guardStart);
    if (guardEnd < guardStart) {
      throw new Error(`Malformed browser initialization guard in ${loaderPath}`);
    }
    const catchStart = source.indexOf('\n} catch (__error) {\n', guardEnd);
    if (catchStart < guardEnd || catchStart > exportStart) {
      throw new Error(`Malformed browser initialization cleanup in ${loaderPath}`);
    }
    const contextDestroyExpression = readInjectedBrowserContextDestroyExpression(
      source,
      loaderPath,
    );
    return `${source.slice(0, catchStart + 1)}${renderBrowserInitializationCleanup(contextDestroyExpression)}
${source.slice(exportStart)}`.replace(/[ \t]+$/gm, '');
  }

  const contextAnchor = 'const __emnapiContext = __emnapiCreateContext()';
  const contextCount = source.split(contextAnchor).length - 1;
  if (contextCount !== 1) {
    throw new Error(`Unable to locate generated browser context creation in ${loaderPath}`);
  }
  const contextStart = source.indexOf(contextAnchor);
  if (contextStart > exportStart) {
    throw new Error(`Generated browser initialization follows its exports in ${loaderPath}`);
  }

  const declarationAnchor = `let __napiInstance
let __wasiModule
let __napiModule`;
  const declarationCount = source.split(declarationAnchor).length - 1;
  const instantiationAnchor = `;({
    instance: __napiInstance,
    module: __wasiModule,
    napiModule: __napiModule,
  } = await __emnapiInstantiateNapiModule`;
  const instantiationCount = source.split(instantiationAnchor).length - 1;
  if (declarationCount !== 1 || instantiationCount !== 1) {
    throw new Error(`Unable to locate generated browser instantiation in ${loaderPath}`);
  }

  const instantiationStart = source.indexOf(instantiationAnchor);
  const tryAnchor = '\ntry {\n';
  const tryStartWithNewline = source.lastIndexOf(tryAnchor, instantiationStart);
  const catchAnchor = '\n} catch (__error) {\n';
  const catchStart = source.indexOf(catchAnchor, instantiationStart);
  if (
    tryStartWithNewline < contextStart ||
    catchStart < instantiationStart ||
    catchStart > exportStart
  ) {
    throw new Error(`Unable to locate generated browser initialization lifecycle in ${loaderPath}`);
  }

  const generatedCatch = source.slice(catchStart + 1, exportStart).trimEnd();
  const legacyGeneratedCatch = `} catch (__error) {
  try {
    await __emnapiContext.destroy()
  } catch (__cleanupError) {
    throw __createInitializationCleanupError(__error, __cleanupError)
  }
  throw __error
}`;
  const upstreamGeneratedCatch = `} catch (__error) {
  const __cleanupErrors = []
  try {
    await __destroyEmnapiContext()
  } catch (__cleanupError) {
    __cleanupErrors.push(__cleanupError)
  }
  if (__cleanupErrors.length > 0) {
    throw __createInitializationCleanupError(__error, __cleanupErrors)
  }
  throw __error
}`;
  const contextDestroyExpression =
    generatedCatch === legacyGeneratedCatch
      ? '__emnapiContext.destroy()'
      : generatedCatch === upstreamGeneratedCatch
        ? '__destroyEmnapiContext()'
        : undefined;
  if (contextDestroyExpression === undefined) {
    throw new Error(`Unexpected generated browser initialization cleanup in ${loaderPath}`);
  }

  const tryStart = tryStartWithNewline + 1;
  const tryBodyStart = tryStart + tryAnchor.length - 1;
  const tryBody = source.slice(tryBodyStart, catchStart).trimEnd();
  const prefix = source.slice(0, tryStart).replace(
    declarationAnchor,
    `let __browserTaskHostRegistration
let __browserTimerHostRegistration
${declarationAnchor}`,
  );

  return `${prefix}try {
${BROWSER_INITIALIZATION_GUARD_START}
${tryBody}
${BROWSER_INITIALIZATION_GUARD_END}
${renderBrowserInitializationCleanup(contextDestroyExpression)}
${source.slice(exportStart)}`;
}

function readInjectedBrowserContextDestroyExpression(
  source: string,
  loaderPath: string,
): '__emnapiContext.destroy()' | '__destroyEmnapiContext()' {
  const expressions = ['__emnapiContext.destroy()', '__destroyEmnapiContext()'] as const;
  const matches = expressions.filter(
    (expression) => source.split(`      () => ${expression},`).length - 1 === 1,
  );
  if (matches.length !== 1) {
    throw new Error(`Malformed browser context cleanup in ${loaderPath}`);
  }
  return matches[0];
}

function renderBrowserInitializationCleanup(
  contextDestroyExpression: '__emnapiContext.destroy()' | '__destroyEmnapiContext()',
): string {
  return `} catch (__error) {
  const __cleanupErrors = []
  const __cleanupSync = (__operation, __message) => {
    const __operationErrors = []
    for (let __attempt = 0; __attempt < 2; __attempt += 1) {
      try {
        __operation()
        return
      } catch (__cleanupError) {
        __operationErrors.push(__cleanupError)
      }
    }
    __cleanupErrors.push(new AggregateError(__operationErrors, __message))
  }
  const __cleanup = async (__operation, __message) => {
    const __operationErrors = []
    for (let __attempt = 0; __attempt < 2; __attempt += 1) {
      try {
        await __operation()
        return
      } catch (__cleanupError) {
        __operationErrors.push(__cleanupError)
      }
    }
    __cleanupErrors.push(new AggregateError(__operationErrors, __message))
  }
  if (__browserTimerHostRegistration !== undefined) {
    __cleanupSync(() => {
      const __binding = __napiModule.exports
      Reflect.apply(__binding.unregisterTimerHost, __binding, [
        __browserTimerHostRegistration.high,
        __browserTimerHostRegistration.low,
      ])
    }, 'Threadless browser timer-host cleanup failed')
  }
  if (__browserTaskHostRegistration !== undefined) {
    __cleanupSync(() => {
      const __binding = __napiModule.exports
      Reflect.apply(__binding.unregisterCurrentThreadTaskHost, __binding, [
        __browserTaskHostRegistration.high,
        __browserTaskHostRegistration.low,
      ])
    }, 'Threadless browser task-host cleanup failed')
  }
  if (__emnapiContext !== undefined) {
    await __cleanup(
      () => ${contextDestroyExpression},
      'Threadless browser context cleanup failed',
    )
  }
  if (__cleanupErrors.length > 0) {
    throw new AggregateError(
      [
        __error,
        new AggregateError(
          __cleanupErrors,
          'Threadless browser initialization cleanup failed',
        ),
      ],
      'Threadless browser initialization failed and cleanup did not complete',
      { cause: __error },
    )
  }
  throw __error
}`;
}

function renderNodeInitializationCleanup(): string {
  return `${NODE_INITIALIZATION_CLEANUP_START}
} catch (__error) {
  const __cleanupErrors = []
  const __cleanupSync = (__operation, __message) => {
    const __operationErrors = []
    for (let __attempt = 0; __attempt < 2; __attempt += 1) {
      try {
        __operation()
        return true
      } catch (__cleanupError) {
        __operationErrors.push(__cleanupError)
      }
    }
    __cleanupErrors.push(new AggregateError(__operationErrors, __message))
    return false
  }
  if (
    typeof __nodeTimerHostRegistration !== 'undefined' &&
    __nodeTimerHostRegistration !== undefined
  ) {
    const __released = __cleanupSync(() => {
      const __binding = __napiModule.exports
      Reflect.apply(__binding.unregisterTimerHost, __binding, [
        __nodeTimerHostRegistration.high,
        __nodeTimerHostRegistration.low,
      ])
    }, 'Threadless Node timer-host cleanup failed')
    if (__released) {
      __nodeTimerHostRegistration = undefined
    }
  }
  if (
    typeof __nodeTaskHostRegistration !== 'undefined' &&
    __nodeTaskHostRegistration !== undefined
  ) {
    const __released = __cleanupSync(() => {
      const __binding = __napiModule.exports
      Reflect.apply(__binding.unregisterCurrentThreadTaskHost, __binding, [
        __nodeTaskHostRegistration.high,
        __nodeTaskHostRegistration.low,
      ])
    }, 'Threadless Node task-host cleanup failed')
    if (__released) {
      __nodeTaskHostRegistration = undefined
    }
  }
  let __cleanupResult
  let __contextCleanupCompleted = false
  const __contextCleanupErrors = []
  for (let __attempt = 0; __attempt < 2; __attempt += 1) {
    try {
      __cleanupResult = __destroyEmnapiContext()
      __contextCleanupCompleted = true
      break
    } catch (__cleanupError) {
      __contextCleanupErrors.push(__cleanupError)
    }
  }
  if (!__contextCleanupCompleted) {
    __cleanupErrors.push(
      new AggregateError(
        __contextCleanupErrors,
        'Threadless Node initialization context cleanup failed',
      ),
    )
    __cleanupSync(
      () => __retainEmnapiContextCleanupListener(),
      'Threadless Node initialization cleanup listener retention failed',
    )
  }
  if (__cleanupResult) {
    void __cleanupResult.then(
      () => {
        try {
          __removeEmnapiContextCleanupListeners()
        } catch (__cleanupError) {
          __preserveCleanupError(__error, __cleanupError)
        }
      },
      (__cleanupError) => {
        __preserveCleanupError(__error, __cleanupError)
        try {
          __retainEmnapiContextCleanupListener()
        } catch (__listenerError) {
          __preserveCleanupError(__error, __listenerError)
        }
      },
    )
  } else if (__contextCleanupCompleted) {
    __cleanupSync(
      () => __removeEmnapiContextCleanupListeners(),
      'Threadless Node initialization cleanup listener removal failed',
    )
  }
  if (__cleanupErrors.length > 0) {
    throw new AggregateError(
      [
        __error,
        new AggregateError(
          __cleanupErrors,
          'Threadless Node initialization cleanup failed',
        ),
      ],
      'Threadless Node initialization failed and cleanup did not complete',
      { cause: __error },
    )
  }
  throw __error
}
${NODE_INITIALIZATION_CLEANUP_END}`;
}

function validateGeneratedNodeLifecycle(source: string, loaderPath: string): void {
  if (
    !source.includes('function __destroyEmnapiContext()') &&
    !source.includes(NODE_INITIALIZATION_CLEANUP_START)
  ) {
    return;
  }
  for (const signature of NODE_LIFECYCLE_HELPER_SIGNATURES) {
    const count = source.split(signature).length - 1;
    if (count !== 1) {
      throw new Error(
        `Unexpected generated Node lifecycle contract in ${loaderPath}: expected exactly one ${JSON.stringify(signature)}, found ${count}`,
      );
    }
  }
  let previousIndex = -1;
  for (const snippet of NODE_LIFECYCLE_CONTRACT_SNIPPETS) {
    const count = source.split(snippet).length - 1;
    const index = source.indexOf(snippet);
    if (count !== 1) {
      throw new Error(
        `Unexpected generated Node lifecycle contract in ${loaderPath}: expected exactly one ${JSON.stringify(snippet)}, found ${count}`,
      );
    }
    if (index <= previousIndex) {
      throw new Error(
        `Unexpected generated Node lifecycle contract in ${loaderPath}: out-of-order ${JSON.stringify(snippet)}`,
      );
    }
    previousIndex = index;
  }
}

function injectNodeHostRegistrationDeclarations(source: string, loaderPath: string): string {
  const taskDeclaration = 'let __nodeTaskHostRegistration';
  const timerDeclaration = 'let __nodeTimerHostRegistration';
  const hasTaskDeclaration = source.includes(taskDeclaration);
  const hasTimerDeclaration = source.includes(timerDeclaration);
  if (hasTaskDeclaration || hasTimerDeclaration) {
    if (!hasTaskDeclaration || !hasTimerDeclaration) {
      throw new Error(`Malformed Node host registration declarations in ${loaderPath}`);
    }
    return source;
  }

  const declarationAnchor = 'let __napiModule';
  const declarationCount = source.split(declarationAnchor).length - 1;
  if (declarationCount !== 1) {
    throw new Error(`Unable to locate generated Node binding declaration in ${loaderPath}`);
  }
  return source.replace(
    declarationAnchor,
    `${taskDeclaration}
${timerDeclaration}
${declarationAnchor}`,
  );
}

function injectNodeInitializationCleanup(
  source: string,
  loaderPath: string,
  generatedCatchAnchor: string,
): string {
  validateGeneratedNodeLifecycle(source, loaderPath);
  source = injectNodeHostRegistrationDeclarations(source, loaderPath);
  const cleanupStartCount = source.split(NODE_INITIALIZATION_CLEANUP_START).length - 1;
  const cleanupEndCount = source.split(NODE_INITIALIZATION_CLEANUP_END).length - 1;
  if (cleanupStartCount !== cleanupEndCount || cleanupStartCount > 1) {
    throw new Error(`Malformed Node initialization cleanup in ${loaderPath}`);
  }
  if (cleanupStartCount === 1) {
    const cleanupStart = source.indexOf(NODE_INITIALIZATION_CLEANUP_START);
    const cleanupEnd = source.indexOf(NODE_INITIALIZATION_CLEANUP_END, cleanupStart);
    if (cleanupEnd < cleanupStart) {
      throw new Error(`Malformed Node initialization cleanup in ${loaderPath}`);
    }
    return (
      source.slice(0, cleanupStart) +
      renderNodeInitializationCleanup() +
      source.slice(cleanupEnd + NODE_INITIALIZATION_CLEANUP_END.length)
    );
  }

  const catchCount = source.split(generatedCatchAnchor).length - 1;
  const exportAnchor = 'module.exports = __napiModule.exports';
  const exportCount = source.split(exportAnchor).length - 1;
  if (catchCount !== 1 || exportCount !== 1) {
    throw new Error(`Unable to locate generated Node initialization cleanup in ${loaderPath}`);
  }
  const catchStart = source.indexOf(generatedCatchAnchor);
  const exportStart = source.indexOf(exportAnchor, catchStart);
  const generatedCatch = source.slice(catchStart, exportStart).trimEnd();
  const expectedGeneratedCatch = `} catch (__error) {
  let __cleanupResult
  let __cleanupFailed = false
  try {
    __cleanupResult = __destroyEmnapiContext()
  } catch (__cleanupError) {
    __cleanupFailed = true
    __preserveCleanupError(__error, __cleanupError)
    try {
      __retainEmnapiContextCleanupListener()
    } catch (__listenerError) {
      __preserveCleanupError(__error, __listenerError)
    }
  }
  if (__cleanupResult) {
    void __cleanupResult.then(
      () => {
        try {
          __removeEmnapiContextCleanupListeners()
        } catch (__cleanupError) {
          __preserveCleanupError(__error, __cleanupError)
        }
      },
      (__cleanupError) => {
        __preserveCleanupError(__error, __cleanupError)
        try {
          __retainEmnapiContextCleanupListener()
        } catch (__listenerError) {
          __preserveCleanupError(__error, __listenerError)
        }
      },
    )
  } else if (!__cleanupFailed) {
    try {
      __removeEmnapiContextCleanupListeners()
    } catch (__cleanupError) {
      __preserveCleanupError(__error, __cleanupError)
    }
  }
  throw __error
}`;
  if (generatedCatch !== expectedGeneratedCatch) {
    throw new Error(`Unexpected generated Node initialization cleanup in ${loaderPath}`);
  }
  return `${source.slice(0, catchStart)}${renderNodeInitializationCleanup()}
${source.slice(exportStart)}`;
}

export function rewriteThreadlessMemoryDescriptor(
  source: string,
  loaderPath: string,
  { initialMemory, maximumMemory }: WasmConfig,
): string {
  const memoryPattern =
    /(((?:(?:const|let)\s+)?__wasmMemory\s*=\s*new WebAssembly\.Memory\(\{\s*\n\s*initial:\s*)\d+(,\s*\n\s*maximum:\s*)\d+)/;
  const memoryMatches = [...source.matchAll(new RegExp(memoryPattern.source, 'g'))];
  if (memoryMatches.length !== 1) {
    throw new Error(`Unable to locate generated threadless memory descriptor in ${loaderPath}`);
  }
  return source.replace(
    memoryPattern,
    (_match, _descriptor: string, prefix: string, separator: string) =>
      `${prefix}${initialMemory}${separator}${maximumMemory}`,
  );
}

export function injectCurrentThreadHostBootstrap(
  source: string,
  loaderPath: string,
  bootstrapAnchor: string,
  browserInitializationGuard: boolean,
  { initialMemory, maximumMemory }: WasmConfig,
): string {
  const startCount = source.split(CURRENT_THREAD_HOST_BOOTSTRAP_START).length - 1;
  const endCount = source.split(CURRENT_THREAD_HOST_BOOTSTRAP_END).length - 1;
  if (startCount !== endCount || startCount > 1) {
    throw new Error(`Malformed CurrentThread host bootstrap in ${loaderPath}`);
  }
  if (startCount === 1) {
    const start = source.indexOf(CURRENT_THREAD_HOST_BOOTSTRAP_START);
    const end = source.indexOf(CURRENT_THREAD_HOST_BOOTSTRAP_END, start);
    if (end < start) {
      throw new Error(`Malformed CurrentThread host bootstrap in ${loaderPath}`);
    }
    source =
      source.slice(0, start) +
      source.slice(end + CURRENT_THREAD_HOST_BOOTSTRAP_END.length).replace(/^\n+/, '');
  }

  source = rewriteThreadlessMemoryDescriptor(source, loaderPath, {
    initialMemory,
    maximumMemory,
  });

  if (browserInitializationGuard) {
    source = injectBrowserInitializationGuard(source, loaderPath, bootstrapAnchor);
  } else {
    source = injectNodeInitializationCleanup(source, loaderPath, bootstrapAnchor);
  }

  const effectiveBootstrapAnchor = browserInitializationGuard
    ? BROWSER_INITIALIZATION_GUARD_END
    : NODE_INITIALIZATION_CLEANUP_START;
  const anchorCount = source.split(effectiveBootstrapAnchor).length - 1;
  if (anchorCount !== 1) {
    throw new Error(`Unable to locate generated CurrentThread bootstrap anchor in ${loaderPath}`);
  }
  const anchor = source.indexOf(effectiveBootstrapAnchor);
  const bootstrap = renderCurrentThreadHostBootstrap(browserInitializationGuard);
  return `${source.slice(0, anchor)}${bootstrap}\n${source.slice(anchor)}`;
}

export function generateWorkerdLoader(): boolean {
  if (!existsSync(DEFERRED_LOADER)) return false;
  const memoryConfig = readMemoryConfig();
  const threadlessDeclarationPath = WASI_DECLARATIONS.threadless;
  if (!existsSync(threadlessDeclarationPath)) {
    throw new Error(
      `Generated threadless WASI declaration not found: ${threadlessDeclarationPath}`,
    );
  }
  const threadlessDeclaration = readFileSync(threadlessDeclarationPath, 'utf8');
  const generatedLoaders = CURRENT_THREAD_LOADERS.map((loader) => {
    if (!existsSync(loader.path)) {
      throw new Error(`Generated threadless WASI loader not found: ${loader.path}`);
    }
    return {
      path: loader.path,
      source: injectCurrentThreadHostBootstrap(
        readFileSync(loader.path, 'utf8'),
        loader.path,
        loader.bootstrapAnchor,
        loader.browserInitializationGuard,
        memoryConfig,
      ),
    };
  });

  writeFileSync(DEFERRED_LOADER, renderWorkerdLoader(memoryConfig));
  writeFileSync(DEFERRED_LOADER_DECLARATION, renderWorkerdLoaderDeclaration());
  writeFileSync(threadlessDeclarationPath, threadlessDeclaration);
  for (const loader of generatedLoaders) {
    writeFileSync(loader.path, loader.source);
  }
  return true;
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
  } else if (!generateWorkerdLoader()) {
    throw new Error(`Deferred WASI loader not found: ${DEFERRED_LOADER}`);
  }
}
