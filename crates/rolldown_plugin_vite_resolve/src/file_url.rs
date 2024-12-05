use anyhow::anyhow;
use url::Url;

/// The caller should check if the url has file scheme.
pub fn file_url_str_to_path(url: &str) -> anyhow::Result<String> {
  let url = Url::parse(url)?;
  // it seems url.to_file_path() does not work in some cases
  // https://github.com/servo/rust-url/issues/505
  // implemented the same logic with fileURLToPath in Node.js for now
  file_url_to_path(url)
}

fn file_url_to_path(url: Url) -> anyhow::Result<String> {
  #[cfg(target_family = "wasm")]
  {
    let pathname = url.path();
    // NOTE: should be decided if it's running on Windows or not
    //       for now, decide it if the path has a drive part
    if is_windows_drive_path(&pathname[1..]) {
      get_path_from_url_windows(url)
    } else {
      get_path_from_url_posix(url)
    }
  }
  #[cfg(windows)]
  {
    get_path_from_url_windows(url)
  }
  #[cfg(not(any(windows, target_family = "wasm")))]
  {
    get_path_from_url_posix(url)
  }
}

#[cfg(any(windows, target_family = "wasm"))]
fn get_path_from_url_windows(url: Url) -> anyhow::Result<String> {
  use crate::utils::is_windows_drive_path;
  use cow_utils::CowUtils;

  let pathname = url.path();
  if pathname.contains("%2F") || pathname.contains("%5C") {
    return Err(anyhow!(
      "Invalid file URL path: must not include encoded \\ or / characters {}",
      url
    ));
  }

  let pathname = pathname.cow_replace('/', "\\");
  let pathname = urlencoding::decode(&pathname)?;

  if url.host_str().is_some() {
    // NOTE: this is supported by Node.js, but is not supported by Vite.
    return Err(anyhow!("Invalid file URL: must not contain hostname {}", url));
  }
  let pathname = &pathname[1..];
  if !is_windows_drive_path(pathname) {
    return Err(anyhow!("Invalid file URL: must be absolute {}", url));
  }
  Ok(pathname.to_owned())
}

#[cfg(not(windows))]
fn get_path_from_url_posix(url: Url) -> anyhow::Result<String> {
  if url.host_str().is_some() {
    return Err(anyhow!("Invalid file URL: must not contain hostname {}", url));
  }

  let pathname = url.path();
  if pathname.contains("%2F") {
    return Err(anyhow!(
      "Invalid file URL path: must not include encoded \\ or / characters {}",
      url
    ));
  }

  let pathname = urlencoding::decode(&pathname)?;
  Ok(pathname.into_owned())
}
