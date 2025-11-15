#[derive(ts_rs::TS, serde::Serialize)]
#[ts(export)]
pub struct InputItem {
  /// For `input: { main: './main.js' }`, `./main.js` has the name `main`.
  /// For `input: ['./main.js']`, `./main.js` doesn't have a name.
  pub name: Option<String>,
  /// For `input: { main: './main.js' }`, `./main.js` is the filename.
  pub filename: String,
}

#[derive(ts_rs::TS, serde::Serialize)]
#[ts(export)]
pub struct PluginItem {
  pub name: String,
  pub plugin_id: u32,
}
