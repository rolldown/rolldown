use std::env;

const EXTERNAL_PROVENANCE: &[&str] = &[
  "ROLLDOWN_LINK_BASELINE_BUILD_PROVENANCE_VERSION",
  "ROLLDOWN_LINK_BASELINE_BUILD_GIT_COMMIT",
  "ROLLDOWN_LINK_BASELINE_BUILD_GIT_TREE",
  "ROLLDOWN_LINK_BASELINE_BUILD_GIT_DIRTY",
  "ROLLDOWN_LINK_BASELINE_BUILD_RUSTC",
  "ROLLDOWN_LINK_BASELINE_BUILD_RUSTC_COMMIT_HASH",
  "ROLLDOWN_LINK_BASELINE_BUILD_RUSTC_HOST",
  "ROLLDOWN_LINK_BASELINE_BUILD_RUSTC_LLVM",
  "ROLLDOWN_LINK_BASELINE_BUILD_CARGO",
  "ROLLDOWN_LINK_BASELINE_BUILD_LTO",
  "ROLLDOWN_LINK_BASELINE_BUILD_CODEGEN_UNITS",
  "ROLLDOWN_LINK_BASELINE_BUILD_STRIP",
  "ROLLDOWN_LINK_BASELINE_BUILD_COMMAND",
];

fn main() {
  for variable in EXTERNAL_PROVENANCE {
    println!("cargo:rerun-if-env-changed={variable}");
    export(variable, env::var(variable).unwrap_or_else(|_| "<unverified>".to_string()));
  }

  for variable in ["PROFILE", "OPT_LEVEL", "DEBUG", "TARGET", "HOST", "CARGO_ENCODED_RUSTFLAGS"] {
    println!("cargo:rerun-if-env-changed={variable}");
  }
  export(
    "ROLLDOWN_LINK_BASELINE_BUILD_PROFILE",
    env::var("PROFILE").unwrap_or_else(|_| "<unset>".to_string()),
  );
  export(
    "ROLLDOWN_LINK_BASELINE_BUILD_OPT_LEVEL",
    env::var("OPT_LEVEL").unwrap_or_else(|_| "<unset>".to_string()),
  );
  export(
    "ROLLDOWN_LINK_BASELINE_BUILD_DEBUG",
    env::var("DEBUG").unwrap_or_else(|_| "<unset>".to_string()),
  );
  export(
    "ROLLDOWN_LINK_BASELINE_BUILD_TARGET",
    env::var("TARGET").unwrap_or_else(|_| "<unset>".to_string()),
  );
  export(
    "ROLLDOWN_LINK_BASELINE_BUILD_HOST",
    env::var("HOST").unwrap_or_else(|_| "<unset>".to_string()),
  );
  export(
    "ROLLDOWN_LINK_BASELINE_BUILD_RUSTFLAGS_HEX",
    encode_hex(env::var_os("CARGO_ENCODED_RUSTFLAGS").unwrap_or_default().as_encoded_bytes()),
  );
}

fn export(variable: &str, value: String) {
  assert!(!value.contains('\r') && !value.contains('\n'), "{variable} must be a single-line value");
  println!("cargo:rustc-env={variable}={value}");
}

fn encode_hex(value: &[u8]) -> String {
  const HEX: &[u8; 16] = b"0123456789abcdef";
  let mut output = String::with_capacity(value.len() * 2);
  for byte in value {
    output.push(char::from(HEX[usize::from(byte >> 4)]));
    output.push(char::from(HEX[usize::from(byte & 0x0f)]));
  }
  output
}
