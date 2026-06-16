use anyhow::anyhow;

/// A minimally-parsed `file:` URL.
///
/// rolldown only handles `file://` URLs here (which never carry an
/// internationalized host), so a small ASCII split replaces `url::Url` and
/// avoids pulling the whole `url`/`idna` stack into the binary.
struct FileUrl<'a> {
  /// The original input, used verbatim in error messages.
  original: &'a str,
  /// Authority/host component, empty for the usual `file:///…` form.
  host: &'a str,
  /// Percent-encoded path. Always begins with `/`.
  path: &'a str,
  /// Raw query, without the leading `?`.
  query: Option<&'a str>,
  /// Raw fragment, without the leading `#`.
  fragment: Option<&'a str>,
}

impl<'a> FileUrl<'a> {
  /// The caller should check that `url` has a `file` scheme.
  fn parse(url: &'a str) -> anyhow::Result<Self> {
    let rest = url.strip_prefix("file:").ok_or_else(|| anyhow!("Invalid file URL: {url}"))?;
    // Drop the authority marker (`file://…`); `file:/…` has none.
    let rest = rest.strip_prefix("//").unwrap_or(rest);

    let (rest, fragment) = match rest.split_once('#') {
      Some((head, fragment)) => (head, Some(fragment)),
      None => (rest, None),
    };
    let (rest, query) = match rest.split_once('?') {
      Some((head, query)) => (head, Some(query)),
      None => (rest, None),
    };
    // Everything before the first `/` is the host (empty for `file:///…`).
    let (host, path) = match rest.find('/') {
      Some(index) => rest.split_at(index),
      None => (rest, "/"),
    };

    Ok(Self { original: url, host, path, query, fragment })
  }

  fn has_host(&self) -> bool {
    !self.host.is_empty()
  }
}

/// The caller should check if the url has file scheme.
pub fn file_url_str_to_path_and_postfix(url: &str) -> anyhow::Result<(String, String)> {
  let url = FileUrl::parse(url)?;

  let postfix = {
    let postfix_len =
      url.query.map_or(0, |q| q.len() + 1) + url.fragment.map_or(0, |f| f.len() + 1);

    let mut postfix = String::with_capacity(postfix_len);
    if let Some(q) = url.query {
      postfix.push('?');
      postfix.push_str(q);
    }
    if let Some(f) = url.fragment {
      postfix.push('#');
      postfix.push_str(f);
    }
    postfix
  };

  // it seems url.to_file_path() does not work in some cases
  // https://github.com/servo/rust-url/issues/505
  // implemented the same logic with fileURLToPath in Node.js for now
  let path = file_url_to_path(&url)?;
  Ok((path, postfix))
}

fn file_url_to_path(url: &FileUrl) -> anyhow::Result<String> {
  #[cfg(target_family = "wasm")]
  {
    use crate::utils::is_windows_drive_path;

    // NOTE: should be decided if it's running on Windows or not
    //       for now, decide it if the path has a drive part
    if is_windows_drive_path(&url.path[1..]) {
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
fn get_path_from_url_windows(url: &FileUrl) -> anyhow::Result<String> {
  use crate::utils::is_windows_drive_path;
  use cow_utils::CowUtils;

  let pathname = url.path;
  if pathname.contains("%2F") || pathname.contains("%5C") {
    return Err(anyhow!(
      "Invalid file URL path: must not include encoded \\ or / characters {}",
      url.original
    ));
  }

  let pathname = pathname.cow_replace('/', "\\");
  let pathname = urlencoding::decode(&pathname)?;

  if url.has_host() {
    // NOTE: this is supported by Node.js, but is not supported by Vite.
    return Err(anyhow!("Invalid file URL: must not contain hostname {}", url.original));
  }
  let pathname = &pathname[1..];
  if !is_windows_drive_path(pathname) {
    return Err(anyhow!("Invalid file URL: must be absolute {}", url.original));
  }
  Ok(pathname.to_owned())
}

#[cfg(not(windows))]
fn get_path_from_url_posix(url: &FileUrl) -> anyhow::Result<String> {
  if url.has_host() {
    return Err(anyhow!("Invalid file URL: must not contain hostname {}", url.original));
  }

  let pathname = url.path;
  if pathname.contains("%2F") {
    return Err(anyhow!(
      "Invalid file URL path: must not include encoded \\ or / characters {}",
      url.original
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
