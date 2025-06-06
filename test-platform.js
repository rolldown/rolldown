/**
 * Test script to verify platform detection logic for WASI platforms
 */
import { is_wasi_platform, is_wasi_preview2 } from './crates/rolldown_common/src/wasi_features.js';
import { Platform } from './crates/rolldown_common/src/inner_bundler_options/types/platform.js';

// Mock the Platform enum to match what we defined in Rust
const PlatformMock = {
  Node: 0,
  Browser: 1,
  Neutral: 2,
  Wasi: 3,
  WasiP2: 4,
};

// Test the WASI platform detection functions
console.log('Testing WASI platform detection...');

console.log('\nTesting is_wasi_platform function:');
console.log('Node platform:', is_wasi_platform(PlatformMock.Node), '(should be false)');
console.log('Browser platform:', is_wasi_platform(PlatformMock.Browser), '(should be false)');
console.log('Neutral platform:', is_wasi_platform(PlatformMock.Neutral), '(should be false)');
console.log('WASI Preview 1 platform:', is_wasi_platform(PlatformMock.Wasi), '(should be true)');
console.log('WASI Preview 2 platform:', is_wasi_platform(PlatformMock.WasiP2), '(should be true)');

console.log('\nTesting is_wasi_preview2 function:');
console.log('Node platform:', is_wasi_preview2(PlatformMock.Node), '(should be false)');
console.log('Browser platform:', is_wasi_preview2(PlatformMock.Browser), '(should be false)');
console.log('Neutral platform:', is_wasi_preview2(PlatformMock.Neutral), '(should be false)');
console.log('WASI Preview 1 platform:', is_wasi_preview2(PlatformMock.Wasi), '(should be false)');
console.log('WASI Preview 2 platform:', is_wasi_preview2(PlatformMock.WasiP2), '(should be true)');

// Mock implementation of the functions we defined in Rust
function is_wasi_platform_js(platform) {
  return platform === PlatformMock.Wasi || platform === PlatformMock.WasiP2;
}

function is_wasi_preview2_js(platform) {
  return platform === PlatformMock.WasiP2;
}

// Compare our JavaScript implementation with the expected Rust behavior
console.log('\nVerifying implementation correctness:');
for (const [name, value] of Object.entries(PlatformMock)) {
  const rustWasi = is_wasi_platform(value);
  const jsWasi = is_wasi_platform_js(value);
  console.log(`Platform ${name}: WASI platform detection matches:`, rustWasi === jsWasi);
  
  const rustWasiP2 = is_wasi_preview2(value);
  const jsWasiP2 = is_wasi_preview2_js(value);
  console.log(`Platform ${name}: WASI Preview 2 detection matches:`, rustWasiP2 === jsWasiP2);
} 