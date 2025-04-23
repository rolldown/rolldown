use anyhow::anyhow;
use url::Url;

/// The caller should check if the url has file scheme.
pub fn file_url_str_to_path_and_postfix(url: &str) -> anyhow::Result<(String, String)> {
  let url = Url::parse(url)?;

  let postfix = {
    let postfix_len =
      url.query().map_or(0, |q| q.len() + 1) + url.fragment().map_or(0, |f| f.len() + 1);

    let mut postfix = String::with_capacity(postfix_len);
    if let Some(q) = url.query() {
      postfix.push('?');
      postfix.push_str(q);
    }
    if let Some(f) = url.fragment() {
      postfix.push('#');
      postfix.push_str(f);
    }
    postfix
  };

  // it seems url.to_file_path() does not work in some cases
  // https://github.com/servo/rust-url/issues/505
  // implemented the same logic with fileURLToPath in Node.js for now
  let path = file_url_to_path(url)?;
  Ok((path, postfix))
}

fn file_url_to_path(url: Url) -> anyhow::Result<String> {
  #[cfg(target_family = "wasm")]
  {
    use crate::utils::is_windows_drive_path;

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

  let pathname = urlencoding::decode(pathname)?;
  Ok(pathname.into_owned())
}
