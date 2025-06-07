/**
 * Test script to verify platform string conversion for WASI platforms
 */
import { tryFrom } from './crates/rolldown_common/src/inner_bundler_options/types/platform.js';
import { Platform } from './crates/rolldown_common/src/inner_bundler_options/types/platform.js';
import { get_wasi_target_triple } from './crates/rolldown_common/src/wasi_features.js';

console.log('Testing platform string conversion...\n');

// Test cases for platform string conversion
const testCases = [
  { input: 'node', expected: Platform.Node, expectedName: 'Node' },
  { input: 'browser', expected: Platform.Browser, expectedName: 'Browser' },
  { input: 'neutral', expected: Platform.Neutral, expectedName: 'Neutral' },
  { input: 'wasi', expected: Platform.Wasi, expectedName: 'Wasi' },
  { input: 'wasip1', expected: Platform.Wasi, expectedName: 'Wasi' },
  { input: 'wasip2', expected: Platform.WasiP2, expectedName: 'WasiP2' },
];

// Test the platform string conversion
for (const { input, expected, expectedName } of testCases) {
  try {
    const platform = tryFrom(input);
    const result = platform === expected;
    console.log(`Conversion for '${input}' to ${expectedName}: ${result ? 'PASSED' : 'FAILED'}`);
  } catch (error) {
    console.error(`Error converting '${input}': ${error.message}`);
  }
}

// Test an invalid platform string
try {
  tryFrom('invalid');
  console.log('Invalid platform test: FAILED (should have thrown)');
} catch (error) {
  console.log('Invalid platform test: PASSED (correctly threw error)');
}

console.log('\nTesting WASI target triples:');
console.log(`WASI Preview 1 target: ${get_wasi_target_triple(Platform.Wasi)} (expected: wasm32-wasip1-threads)`);
console.log(`WASI Preview 2 target: ${get_wasi_target_triple(Platform.WasiP2)} (expected: wasm32-wasip2)`);
console.log(`Node platform target (should be null): ${get_wasi_target_triple(Platform.Node)}`); 