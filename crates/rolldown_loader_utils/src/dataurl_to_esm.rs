use anyhow::{Ok, Result};
use rolldown_utils::{base64, dataurl, mime};

pub fn dataurl_to_esm(ext: Option<&str>, base64: &str) -> Result<String> {
  let buf = base64::from_standard_base64(base64)?;
  let mime = mime::guess_mime_type(&ext.map(|ext| format!(".{ext}")).unwrap_or_default(), &buf);
  Ok(
    serde_json::to_string(&dataurl::encode_as_shortest_dataurl(&mime, &buf))
      .map(|text| ["export default ", text.as_str(), ";"].concat())?,
  )
}
