use rolldown_common::{Platform, is_wasi_platform, is_wasi_preview2, get_wasi_target_triple};
use rolldown_error::BuildDiagnostic;
use rolldown_resolver::ResolveOptions;

/// Configures the resolve options for WASI platforms
pub fn configure_wasi_resolve_options(
  resolve_options: &mut ResolveOptions,
  platform: Platform,
) -> Result<(), BuildDiagnostic> {
  if !is_wasi_platform(platform) {
    return Ok(());
  }

  // Add WASI-specific condition names
  let condition_names = resolve_options
    .condition_names
    .get_or_insert_with(Vec::new);
  
  // Add 'wasi' condition for all WASI platforms
  if !condition_names.contains(&"wasi".to_string()) {
    condition_names.push("wasi".to_string());
  }

  // Add platform-specific condition
  match platform {
    Platform::Wasi => {
      if !condition_names.contains(&"wasip1".to_string()) {
        condition_names.push("wasip1".to_string());
      }
    }
    Platform::WasiP2 => {
      if !condition_names.contains(&"wasip2".to_string()) {
        condition_names.push("wasip2".to_string());
      }
      
      // Add component-model condition for WasiP2
      if !condition_names.contains(&"component-model".to_string()) {
        condition_names.push("component-model".to_string());
      }
    }
    _ => {}
  }

  Ok(())
}

/// Gets file extension for WASI outputs based on the platform
pub fn get_wasi_output_extension(platform: Platform) -> Option<&'static str> {
  if !is_wasi_platform(platform) {
    return None;
  }

  match platform {
    Platform::Wasi => Some(".wasm"),
    Platform::WasiP2 => Some(".component.wasm"),
    _ => None,
  }
}

/// Gets the appropriate linker command to use for the platform
pub fn get_wasi_linker_command(platform: Platform) -> Option<&'static str> {
  match platform {
    Platform::Wasi => Some("wasm-ld"),
    Platform::WasiP2 => Some("wasm-component-ld"),
    _ => None,
  }
}

/// Checks if filesystem capabilities need to be handled specially (e.g., adding preopens)
pub fn needs_wasi_filesystem_capabilities(platform: Platform) -> bool {
  is_wasi_platform(platform)
}

/// Adds required environment settings for WASI platform to the build process
pub fn add_wasi_environment_settings(
  environment: &mut std::collections::HashMap<String, String>,
  platform: Platform,
) {
  if !is_wasi_platform(platform) {
    return;
  }

  // Set appropriate target triple
  if let Some(target) = get_wasi_target_triple(platform) {
    environment.insert("RUSTUP_TOOLCHAIN".to_string(), "nightly".to_string());
    environment.insert("TARGET".to_string(), target.to_string());
    
    // For Preview 2, we need component model
    if is_wasi_preview2(platform) {
      environment.insert("RUSTFLAGS".to_string(), "-Z wasm-component-model".to_string());
    }
  }
} 