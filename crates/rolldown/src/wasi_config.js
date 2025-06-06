/**
 * Mock implementation of the wasi_config.rs module from Rust
 */
import { Platform } from '../../rolldown_common/src/inner_bundler_options/types/platform.js';
import { is_wasi_platform, is_wasi_preview2 } from '../../rolldown_common/src/wasi_features.js';

/**
 * Configures the resolve options for WASI platforms
 */
export function configure_wasi_resolve_options(resolveOptions, platform) {
  if (!is_wasi_platform(platform)) {
    return true;
  }

  // Add WASI-specific condition names
  const conditionNames = resolveOptions.condition_names || [];
  
  // Add 'wasi' condition for all WASI platforms
  if (!conditionNames.includes('wasi')) {
    conditionNames.push('wasi');
  }

  // Add platform-specific condition
  if (platform === Platform.Wasi) {
    if (!conditionNames.includes('wasip1')) {
      conditionNames.push('wasip1');
    }
  } else if (platform === Platform.WasiP2) {
    if (!conditionNames.includes('wasip2')) {
      conditionNames.push('wasip2');
    }
    
    // Add component-model condition for WasiP2
    if (!conditionNames.includes('component-model')) {
      conditionNames.push('component-model');
    }
  }

  resolveOptions.condition_names = conditionNames;
  return true;
}

/**
 * Gets file extension for WASI outputs based on the platform
 */
export function get_wasi_output_extension(platform) {
  if (!is_wasi_platform(platform)) {
    return null;
  }

  if (platform === Platform.Wasi) {
    return '.wasm';
  } else if (platform === Platform.WasiP2) {
    return '.component.wasm';
  }
  
  return null;
}

/**
 * Gets the appropriate linker command to use for the platform
 */
export function get_wasi_linker_command(platform) {
  if (platform === Platform.Wasi) {
    return 'wasm-ld';
  } else if (platform === Platform.WasiP2) {
    return 'wasm-component-ld';
  }
  
  return null;
}

/**
 * Checks if filesystem capabilities need to be handled specially (e.g., adding preopens)
 */
export function needs_wasi_filesystem_capabilities(platform) {
  return is_wasi_platform(platform);
}

/**
 * Adds required environment settings for WASI platform to the build process
 */
export function add_wasi_environment_settings(environment, platform) {
  if (!is_wasi_platform(platform)) {
    return;
  }

  // Add RUSTUP_TOOLCHAIN=nightly
  environment['RUSTUP_TOOLCHAIN'] = 'nightly';
  
  // For Preview 2, we need component model
  if (is_wasi_preview2(platform)) {
    environment['RUSTFLAGS'] = '-Z wasm-component-model';
  }
} 