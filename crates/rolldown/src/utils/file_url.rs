/// Prefixes recognized on `import.meta.<prefix><referenceId>` file URL references.
///
/// `ROLLDOWN_FILE_URL_` is a rolldown-specific alias of Rollup's `ROLLUP_FILE_URL_`.
const ROLLDOWN_FILE_URL_PREFIX: &str = "ROLLDOWN_FILE_URL_";
const ROLLUP_FILE_URL_PREFIX: &str = "ROLLUP_FILE_URL_";

/// Reference ids are produced by `FileEmitter::assign_reference_id` as the base64url
/// encoding of a 128-bit xxhash (with `-` swapped for `$`), so they are always exactly
/// this many characters. The optional `_<urlId>` part of
/// `import.meta.ROLLDOWN_FILE_URL_<referenceId>_<urlId>` therefore begins right after them.
const REFERENCE_ID_LEN: usize = 22;

/// A parsed `import.meta.<prefix><referenceId>[_<urlId>]` reference.
#[derive(Debug, PartialEq, Eq)]
pub struct FileUrlReference<'a> {
  pub reference_id: &'a str,
  pub url_id: Option<&'a str>,
}

/// Parses `name` when it begins with a recognized file URL prefix.
///
/// For `ROLLDOWN_FILE_URL_<referenceId>`, an optional `_<urlId>` suffix is split off and
/// exposed as [`FileUrlReference::url_id`]. The `ROLLUP_FILE_URL_` alias is parsed for
/// backward compatibility with Rollup and never carries a `urlId`.
pub fn strip_file_url_prefix(name: &str) -> Option<FileUrlReference<'_>> {
  if let Some(rest) = name.strip_prefix(ROLLDOWN_FILE_URL_PREFIX) {
    // Reference ids are fixed-length, so the `_<urlId>` suffix (if any) begins right after
    // them. `rest` is ASCII (base64url + `$`), so byte indexing lands on char boundaries.
    if rest.len() > REFERENCE_ID_LEN && rest.as_bytes()[REFERENCE_ID_LEN] == b'_' {
      let url_id = &rest[REFERENCE_ID_LEN + 1..];
      return Some(FileUrlReference {
        reference_id: &rest[..REFERENCE_ID_LEN],
        url_id: if url_id.is_empty() { None } else { Some(url_id) },
      });
    }
    return Some(FileUrlReference { reference_id: rest, url_id: None });
  }
  name
    .strip_prefix(ROLLUP_FILE_URL_PREFIX)
    .map(|reference_id| FileUrlReference { reference_id, url_id: None })
}

/// Whether `name` begins with a recognized file URL prefix.
pub fn starts_with_file_url_prefix(name: &str) -> bool {
  strip_file_url_prefix(name).is_some()
}

#[cfg(test)]
mod test {
  use super::{FileUrlReference, starts_with_file_url_prefix, strip_file_url_prefix};

  #[test]
  fn strips_rolldown_prefix_without_url_id() {
    // A 22-char reference id with no `_<urlId>` suffix.
    let reference_id = "aaaaaaaaaaaaaaaaaaaaaa";
    assert_eq!(
      strip_file_url_prefix(&format!("ROLLDOWN_FILE_URL_{reference_id}")),
      Some(FileUrlReference { reference_id, url_id: None })
    );
    assert_eq!(
      strip_file_url_prefix(&format!("ROLLDOWN_FILE_URL_{reference_id}_")),
      Some(FileUrlReference { reference_id, url_id: None })
    );
  }

  #[test]
  fn strips_rolldown_prefix_with_url_id() {
    let reference_id = "aaaaaaaaaaaaaaaaaaaaaa";
    assert_eq!(
      strip_file_url_prefix(&format!("ROLLDOWN_FILE_URL_{reference_id}_myUrlId")),
      Some(FileUrlReference { reference_id, url_id: Some("myUrlId") })
    );
    // A `urlId` may itself contain underscores.
    assert_eq!(
      strip_file_url_prefix(&format!("ROLLDOWN_FILE_URL_{reference_id}_a_b_c")),
      Some(FileUrlReference { reference_id, url_id: Some("a_b_c") })
    );
  }

  #[test]
  fn rollup_prefix_never_carries_url_id() {
    let reference_id = "aaaaaaaaaaaaaaaaaaaaaa_myUrlId";
    assert_eq!(
      strip_file_url_prefix(&format!("ROLLUP_FILE_URL_{reference_id}")),
      Some(FileUrlReference { reference_id, url_id: None })
    );
  }

  #[test]
  fn shorter_reference_ids_have_no_url_id() {
    // Reference ids shorter than the fixed length (e.g. in unit tests) are taken verbatim.
    assert_eq!(
      strip_file_url_prefix("ROLLDOWN_FILE_URL_abc123"),
      Some(FileUrlReference { reference_id: "abc123", url_id: None })
    );
  }

  #[test]
  fn ignores_unrecognized_names() {
    assert!(strip_file_url_prefix("url").is_none());
    assert!(!starts_with_file_url_prefix("url"));
    assert!(starts_with_file_url_prefix("ROLLDOWN_FILE_URL_abc123"));
    assert!(starts_with_file_url_prefix("ROLLUP_FILE_URL_abc123"));
  }
}
