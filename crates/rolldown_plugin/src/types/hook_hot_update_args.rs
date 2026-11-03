use arcstr::ArcStr;
use rolldown_common::WatcherChangeKind;

/// Arguments for the dev-only `hotUpdate` hook.
///
/// The hook runs once per changed file during an HMR update, after the engine maps the file to
/// its default affected modules and before those modules are re-fetched. Module ids are raw
/// module ids (absolute paths or virtual ids), never the escaped stable-id encoding used by
/// client-facing payloads.
#[derive(Debug)]
pub struct HookHotUpdateArgs {
  pub kind: WatcherChangeKind,
  /// Normalized absolute path of the changed file.
  pub file: ArcStr,
  /// The affected module ids as currently computed. Plugins earlier in the chain may have
  /// replaced the engine's default mapping.
  pub modules: Vec<ArcStr>,
}
