use anyhow::anyhow;
use iri_string::types::UriStr;

/// `true` if the URL has a non-empty authority (host). `file:///…` has an empty
/// authority, which we treat as "no host" to match the previous `url` behaviour.
fn has_host(url: &UriStr) -> bool {
  url.authority_str().is_some_and(|authority| !authority.is_empty())
}

/// The caller should check if the url has file scheme.
pub fn file_url_str_to_path_and_postfix(url: &str) -> anyhow::Result<(String, String)> {
  let parsed = UriStr::new(url).map_err(|_| anyhow!("Invalid file URL: {url}"))?;

  let query = parsed.query().map(iri_string::types::UriQueryStr::as_str);
  let fragment = parsed.fragment().map(iri_string::types::UriFragmentStr::as_str);
  let postfix = {
    let postfix_len = query.map_or(0, |q| q.len() + 1) + fragment.map_or(0, |f| f.len() + 1);

    let mut postfix = String::with_capacity(postfix_len);
    if let Some(q) = query {
      postfix.push('?');
      postfix.push_str(q);
    }
    if let Some(f) = fragment {
      postfix.push('#');
      postfix.push_str(f);
    }
    postfix
  };

  // it seems url.to_file_path() does not work in some cases
  // https://github.com/servo/rust-url/issues/505
  // implemented the same logic with fileURLToPath in Node.js for now
  let path = file_url_to_path(parsed, url)?;
  Ok((path, postfix))
}

fn file_url_to_path(url: &UriStr, original: &str) -> anyhow::Result<String> {
  #[cfg(target_family = "wasm")]
  {
    use crate::utils::is_windows_drive_path;

    // NOTE: should be decided if it's running on Windows or not
    //       for now, decide it if the path has a drive part
    if is_windows_drive_path(url.path_str().get(1..).unwrap_or("")) {
      get_path_from_url_windows(url, original)
    } else {
      get_path_from_url_posix(url, original)
    }
  }
  #[cfg(windows)]
  {
    get_path_from_url_windows(url, original)
  }
  #[cfg(not(any(windows, target_family = "wasm")))]
  {
    get_path_from_url_posix(url, original)
  }
}

#[cfg(any(windows, target_family = "wasm"))]
fn get_path_from_url_windows(url: &UriStr, original: &str) -> anyhow::Result<String> {
  use crate::utils::is_windows_drive_path;
  use cow_utils::CowUtils;

  let pathname = url.path_str();
  if pathname.contains("%2F") || pathname.contains("%5C") {
    return Err(anyhow!(
      "Invalid file URL path: must not include encoded \\ or / characters {original}"
    ));
  }

  let pathname = pathname.cow_replace('/', "\\");
  let pathname = urlencoding::decode(&pathname)?;

  if has_host(url) {
    // NOTE: this is supported by Node.js, but is not supported by Vite.
    return Err(anyhow!("Invalid file URL: must not contain hostname {original}"));
  }
  let pathname = &pathname[1..];
  if !is_windows_drive_path(pathname) {
    return Err(anyhow!("Invalid file URL: must be absolute {original}"));
  }
  Ok(pathname.to_owned())
}

#[cfg(not(windows))]
fn get_path_from_url_posix(url: &UriStr, original: &str) -> anyhow::Result<String> {
  if has_host(url) {
    return Err(anyhow!("Invalid file URL: must not contain hostname {original}"));
  }

  let pathname = url.path_str();
  if pathname.contains("%2F") {
    return Err(anyhow!(
      "Invalid file URL path: must not include encoded \\ or / characters {original}"
    ));
  }

  let pathname = urlencoding::decode(pathname)?;
  Ok(pathname.into_owned())
}

#[cfg(all(test, not(windows), not(target_family = "wasm")))]
mod tests {
  use super::file_url_str_to_path_and_postfix;

  #[test]
  fn plain_file_url() {
    let (path, postfix) = file_url_str_to_path_and_postfix("file:///foo/bar.js").unwrap();
    assert_eq!(path, "/foo/bar.js");
    assert_eq!(postfix, "");
  }

  #[test]
  fn query_and_fragment_become_postfix() {
    let (path, postfix) = file_url_str_to_path_and_postfix("file:///foo/bar.js?v=1#frag").unwrap();
    assert_eq!(path, "/foo/bar.js");
    assert_eq!(postfix, "?v=1#frag");
  }

  #[test]
  fn percent_encoded_path_is_decoded() {
    let (path, _) = file_url_str_to_path_and_postfix("file:///foo/a%20b.js").unwrap();
    assert_eq!(path, "/foo/a b.js");
  }

  #[test]
  fn hostname_is_rejected() {
    assert!(file_url_str_to_path_and_postfix("file://localhost/foo.js").is_err());
  }

  #[test]
  fn encoded_slash_is_rejected() {
    assert!(file_url_str_to_path_and_postfix("file:///foo%2Fbar.js").is_err());
  }

  #[test]
  fn single_slash_form() {
    let (path, _) = file_url_str_to_path_and_postfix("file:/foo/bar.js").unwrap();
    assert_eq!(path, "/foo/bar.js");
  }
}
