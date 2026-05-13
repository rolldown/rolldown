use napi::Either;
use rolldown_common::CheckSetting;

/// Convert a JS-side `false | 'warn' | 'error'` value into a `CheckSetting`. The
/// valibot validator runs first on the JS side, so any other value reaching this point
/// means a caller bypassed the validator — panic to surface the bug loudly.
pub fn either_to_check_setting(value: Either<bool, String>) -> CheckSetting {
  match value {
    Either::A(false) => CheckSetting::Off,
    Either::A(true) => {
      panic!("invalid check severity: `true` is not accepted, use 'warn' or 'error' instead")
    }
    Either::B(s) => match s.as_str() {
      "warn" => CheckSetting::Warn,
      "error" => CheckSetting::Error,
      other => panic!("invalid check severity: expected 'warn' or 'error', got {other:?}"),
    },
  }
}
