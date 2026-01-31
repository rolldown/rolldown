#!/usr/bin/env node
/**
 * Post-build script to patch WASI browser bindings for Safari compatibility
 *
 * Safari doesn't support cloning WebAssembly.Module objects via postMessage.
 * This script patches the generated files to detect this and send raw WASM bytes instead.
 */

import { readFileSync, writeFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';

const __dirname = dirname(fileURLToPath(import.meta.url));
const srcDir = join(__dirname, 'src');

// Patch rolldown-binding.wasi-browser.js
const wasiBindingPath = join(srcDir, 'rolldown-binding.wasi-browser.js');
let wasiBindingContent = readFileSync(wasiBindingPath, 'utf-8');

// Find the location after __wasmFile is defined
const wasmFileMarker = 'const __wasmFile = await fetch(__wasmUrl).then((res) => res.arrayBuffer())';
const wasmFileIndex = wasiBindingContent.indexOf(wasmFileMarker);

if (wasmFileIndex === -1) {
  throw new Error('Could not find __wasmFile marker in rolldown-binding.wasi-browser.js');
}

// Insert Safari detection code
const safariDetectionCode = `

// Check if WebAssembly.Module can be cloned (Safari doesn't support this)
let __supportsModuleClone = false
try {
  const testModule = new WebAssembly.Module(new Uint8Array([0, 97, 115, 109, 1, 0, 0, 0]))
  new MessageChannel().port1.postMessage(testModule)
  __supportsModuleClone = true
} catch {
  // Safari throws DataCloneError
  __supportsModuleClone = false
}
`;

wasiBindingContent =
  wasiBindingContent.slice(0, wasmFileIndex + wasmFileMarker.length) +
  safariDetectionCode +
  wasiBindingContent.slice(wasmFileIndex + wasmFileMarker.length);

// Find the onCreateWorker function and patch it
const onCreateWorkerMarker =
  "worker.addEventListener('message', __wasmCreateOnMessageForFsProxy(__fs))";
const onCreateWorkerIndex = wasiBindingContent.indexOf(onCreateWorkerMarker);

if (onCreateWorkerIndex === -1) {
  throw new Error('Could not find onCreateWorker marker in rolldown-binding.wasi-browser.js');
}

const workerPatchCode = `
    
    // Store info about module clone support on worker
    worker.__supportsModuleClone = __supportsModuleClone
    worker.__wasmFile = __wasmFile`;

wasiBindingContent =
  wasiBindingContent.slice(0, onCreateWorkerIndex + onCreateWorkerMarker.length) +
  workerPatchCode +
  wasiBindingContent.slice(onCreateWorkerIndex + onCreateWorkerMarker.length);

// Find the end of instantiation and add PThread patch
const instantiateEndMarker = '})';
let lastInstantiateEndIndex = wasiBindingContent.lastIndexOf(instantiateEndMarker);

// Find the correct closing brace (the one after beforeInit)
const beforeInitMarker = 'beforeInit({ instance })';
const beforeInitIndex = wasiBindingContent.indexOf(beforeInitMarker);
if (beforeInitIndex !== -1) {
  // Find the closing brace after beforeInit
  let braceCount = 0;
  let foundBeforeInit = false;
  for (let i = beforeInitIndex; i < wasiBindingContent.length; i++) {
    if (wasiBindingContent[i] === '{') {
      braceCount++;
      foundBeforeInit = true;
    } else if (wasiBindingContent[i] === '}') {
      braceCount--;
      if (foundBeforeInit && braceCount === -1) {
        // Found the closing brace of the instantiate options
        lastInstantiateEndIndex = i;
        break;
      }
    }
  }
}

const pthreadPatchCode = `

// Patch PThread.loadWasmModuleToWorker to send raw bytes on Safari
if (!__supportsModuleClone && __napiModule.PThread) {
  const originalLoadWasmModuleToWorker = __napiModule.PThread.loadWasmModuleToWorker
  __napiModule.PThread.loadWasmModuleToWorker = function(worker, sab) {
    // Intercept and send raw bytes instead of module
    const originalPostMessage = worker.postMessage
    worker.postMessage = function(message) {
      if (message && message.__emnapi__ && message.__emnapi__.type === 'load') {
        // Replace wasmModule with raw bytes
        const modifiedMessage = {
          ...message,
          __emnapi__: {
            ...message.__emnapi__,
            payload: {
              ...message.__emnapi__.payload,
              wasmModule: null,
              wasmBytes: worker.__wasmFile,
              wasmMemory: message.__emnapi__.payload.wasmMemory
            }
          }
        }
        return originalPostMessage.call(this, modifiedMessage)
      }
      return originalPostMessage.apply(this, arguments)
    }
    
    const result = originalLoadWasmModuleToWorker.call(this, worker, sab)
    
    // Restore original postMessage after load
    worker.postMessage = originalPostMessage
    
    return result
  }
}
`;

wasiBindingContent =
  wasiBindingContent.slice(0, lastInstantiateEndIndex + instantiateEndMarker.length) +
  pthreadPatchCode +
  wasiBindingContent.slice(lastInstantiateEndIndex + instantiateEndMarker.length);

writeFileSync(wasiBindingPath, wasiBindingContent);
console.log('✓ Patched rolldown-binding.wasi-browser.js for Safari compatibility');

// Patch wasi-worker-browser.mjs
const wasiWorkerPath = join(srcDir, 'wasi-worker-browser.mjs');
let wasiWorkerContent = readFileSync(wasiWorkerPath, 'utf-8');

// Find onLoad function parameters
const onLoadMarker = 'onLoad({ wasmModule, wasmMemory })';
const onLoadIndex = wasiWorkerContent.indexOf(onLoadMarker);

if (onLoadIndex === -1) {
  throw new Error('Could not find onLoad marker in wasi-worker-browser.mjs');
}

// Replace parameters to include wasmBytes
wasiWorkerContent = wasiWorkerContent.replace(
  onLoadMarker,
  'onLoad({ wasmModule, wasmBytes, wasmMemory })',
);

// Find where wasmModule is used in instantiateNapiModuleSync
const instantiateMarker = 'return instantiateNapiModuleSync(wasmModule,';
const instantiateIndex = wasiWorkerContent.indexOf(instantiateMarker);

if (instantiateIndex === -1) {
  throw new Error('Could not find instantiate marker in wasi-worker-browser.mjs');
}

// Add module compilation fallback
const moduleCompilationCode = `
    
    // If we received raw bytes instead of a module (Safari fallback), compile it
    const moduleToUse = wasmModule || new WebAssembly.Module(wasmBytes)
    
    return instantiateNapiModuleSync(moduleToUse,`;

wasiWorkerContent =
  wasiWorkerContent.slice(0, instantiateIndex) +
  moduleCompilationCode.trimStart() +
  wasiWorkerContent.slice(instantiateIndex + instantiateMarker.length);

writeFileSync(wasiWorkerPath, wasiWorkerContent);
console.log('✓ Patched wasi-worker-browser.mjs for Safari compatibility');

console.log('✓ Safari compatibility patches applied successfully');
