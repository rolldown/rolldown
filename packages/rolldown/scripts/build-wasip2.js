// Build script for WASI Preview 2 binding
const fs = require('fs');
const path = require('path');
const { execSync } = require('child_process');

const ROOT = path.join(__dirname, '..');

console.log('Building WASI Preview 2 binding...');

// Ensure the npm/wasm32-wasip2 directory exists
const wasip2Dir = path.join(ROOT, 'npm', 'wasm32-wasip2');
if (!fs.existsSync(wasip2Dir)) {
  fs.mkdirSync(wasip2Dir, { recursive: true });
}

// Source WASM files from the target directory (using wasip1 as the target)
const targetDir = path.join(ROOT, 'target-wasm', 'wasm32-wasip1', 'release-wasip2');
const sourceWasm = path.join(targetDir, 'rolldown_binding.wasm');

if (!fs.existsSync(sourceWasm)) {
  console.error('Error: Could not find the built WASI Preview 2 WASM binary.');
  console.error(`Expected file: ${sourceWasm}`);
  console.error('Make sure you have run the build command correctly.');
  process.exit(1);
}

// For testing purposes, just copy the WASM file directly
// Later, when component model is available, we'll use wasm-tools to convert it
const wasmOutputPath = path.join(wasip2Dir, 'binding.wasm');
console.log(`Copying WASM file to ${wasmOutputPath}`);
fs.copyFileSync(sourceWasm, wasmOutputPath);

// Create types file
const typesFile = path.join(wasip2Dir, 'binding.wasm.d.ts');
console.log(`Creating types file at ${typesFile}`);
fs.writeFileSync(
  typesFile,
  `export interface RolldownWasip2Exports {
  bundle: (options: string) => string;
  version: () => string;
}

declare const exports: RolldownWasip2Exports;
export default exports;
`
);

console.log('WASI Preview 2 binding has been built successfully!'); 