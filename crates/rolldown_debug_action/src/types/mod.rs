#[derive(ts_rs::TS, serde::Serialize)]
#[ts(export)]
pub struct InputItem {
  /// For `input: { main: './main.js' }`, `./main.js` has the name `main`.
  /// For `input: ['./main.js']`, `./main.js` doesn't have a name.
  pub name: Option<String>,
  pub import: String,
}

#[derive(ts_rs::TS, serde::Serialize)]
#[ts(export)]
pub struct PluginItem {
  pub name: String,
  pub index: u32,
}
