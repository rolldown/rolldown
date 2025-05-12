use std::path::{Component, Path};

/// Extracts the longest common path from two given file paths.
///
/// # Arguments
///
/// * `path1` - A string slice representing the first file path.
/// * `path2` - A string slice representing the second file path.
///
/// # Returns
///
/// A `String` representing the longest common path.  If there is no common path,
/// an empty string is returned.
pub fn extract_longest_common_path(path1: &str, path2: &str) -> String {
  let path1 = Path::new(path1);
  let path2 = Path::new(path2);

  let mut components1 = path1.components().peekable();
  let mut components2 = path2.components().peekable();
  let mut common_path = String::new();

  while let (Some(&comp1), Some(&comp2)) = (components1.peek(), components2.peek()) {
    if comp1 == comp2 {
      // Append the component to the common path.  We need to convert
      // the component to a string.
      match comp1 {
        Component::Prefix(_) | Component::RootDir | Component::CurDir | Component::ParentDir => {
          common_path.push_str(comp1.as_os_str().to_string_lossy().as_ref());
        }
        Component::Normal(s) => {
          common_path.push_str(s.to_str().unwrap_or("")); //Handle the case where the component is not valid utf-8.
          common_path.push_str(std::path::MAIN_SEPARATOR_STR);
        }
      }

      components1.next();
      components2.next();
    } else {
      break;
    }
  }
  //Remove the last separator if it exists and the path is not just the root.
  if common_path.len() > 1 && common_path.ends_with(std::path::MAIN_SEPARATOR_STR) {
    common_path.pop();
  }
  common_path
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_extract_longest_common_path() {
    let path1 = "/home/user/documents/report.txt";
    let path2 = "/home/user/pictures/image.jpg";
    let path3 = "/home/user/documents/presentation.pdf";
    let path4 = "/home/user";
    let path5 = "/home/user/documents";
    let path6 = "/usr/local/bin";
    let path7 = "/usr/local";
    let path8 = "/";
    let path9 = "/";

    #[cfg(not(windows))]
    {
      assert_eq!(extract_longest_common_path(path1, path2), "/home/user");
      assert_eq!(extract_longest_common_path(path1, path3), "/home/user/documents");
      assert_eq!(extract_longest_common_path(path4, path5), "/home/user");
      assert_eq!(extract_longest_common_path(path6, path7), "/usr/local");
      assert_eq!(extract_longest_common_path(path1, path6), "/");
      assert_eq!(extract_longest_common_path(path8, path9), "/");
    }
    #[cfg(windows)]
    {
      let path_mixed1 = "C:\\Users\\user\\Documents\\report.txt";
      let path_mixed2 = "/Users/user/Documents/report.txt";
      assert_eq!(extract_longest_common_path(path_mixed1, path_mixed2), "");
    }
  }
}
