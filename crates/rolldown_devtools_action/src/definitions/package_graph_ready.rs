#[derive(ts_rs::TS, serde::Serialize)]
#[ts(export)]
pub struct PackageGraphReady {
  #[ts(type = "'PackageGraphReady'")]
  pub action: &'static str,
  pub packages: Vec<PackageInfo>,
}

#[derive(ts_rs::TS, serde::Serialize)]
#[ts(export)]
pub struct PackageInfo {
  pub package_id: String,
  pub name: Option<String>,
  pub version: Option<String>,
  pub package_json_path: String,
  pub package_root: String,
}
