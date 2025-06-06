/**
 * Test script to verify WASI configuration functions
 */
import { Platform } from './crates/rolldown_common/src/inner_bundler_options/types/platform.js';
import { 
  configure_wasi_resolve_options,
  get_wasi_output_extension,
  get_wasi_linker_command,
  needs_wasi_filesystem_capabilities,
  add_wasi_environment_settings
} from './crates/rolldown/src/wasi_config.js';

console.log('Testing WASI configuration...\n');

// Test configure_wasi_resolve_options
console.log('Testing configure_wasi_resolve_options:');
const testResolveOptions = (platform) => {
  const resolveOptions = { condition_names: [] };
  configure_wasi_resolve_options(resolveOptions, platform);
  console.log(`Platform ${Object.keys(Platform).find(key => Platform[key] === platform)}:`, 
    resolveOptions.condition_names);
  return resolveOptions;
};

const nodeOptions = testResolveOptions(Platform.Node);
const browserOptions = testResolveOptions(Platform.Browser);
const wasiOptions = testResolveOptions(Platform.Wasi);
const wasip2Options = testResolveOptions(Platform.WasiP2);

// Test get_wasi_output_extension
console.log('\nTesting get_wasi_output_extension:');
console.log('Node platform extension:', get_wasi_output_extension(Platform.Node));
console.log('Browser platform extension:', get_wasi_output_extension(Platform.Browser));
console.log('WASI Preview 1 extension:', get_wasi_output_extension(Platform.Wasi));
console.log('WASI Preview 2 extension:', get_wasi_output_extension(Platform.WasiP2));

// Test get_wasi_linker_command
console.log('\nTesting get_wasi_linker_command:');
console.log('Node platform linker:', get_wasi_linker_command(Platform.Node));
console.log('Browser platform linker:', get_wasi_linker_command(Platform.Browser));
console.log('WASI Preview 1 linker:', get_wasi_linker_command(Platform.Wasi));
console.log('WASI Preview 2 linker:', get_wasi_linker_command(Platform.WasiP2));

// Test needs_wasi_filesystem_capabilities
console.log('\nTesting needs_wasi_filesystem_capabilities:');
console.log('Node platform:', needs_wasi_filesystem_capabilities(Platform.Node));
console.log('Browser platform:', needs_wasi_filesystem_capabilities(Platform.Browser));
console.log('WASI Preview 1 platform:', needs_wasi_filesystem_capabilities(Platform.Wasi));
console.log('WASI Preview 2 platform:', needs_wasi_filesystem_capabilities(Platform.WasiP2));

// Test add_wasi_environment_settings
console.log('\nTesting add_wasi_environment_settings:');
const testEnvironment = (platform) => {
  const env = {};
  add_wasi_environment_settings(env, platform);
  console.log(`Platform ${Object.keys(Platform).find(key => Platform[key] === platform)}:`, env);
  return env;
};

const nodeEnv = testEnvironment(Platform.Node);
const browserEnv = testEnvironment(Platform.Browser);
const wasiEnv = testEnvironment(Platform.Wasi);
const wasip2Env = testEnvironment(Platform.WasiP2); 