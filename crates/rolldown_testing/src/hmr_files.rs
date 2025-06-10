use std::{
  fs,
  path::{Path, PathBuf},
  sync::LazyLock,
};

use regex::Regex;

static HMR_EDIT_FILENAME_RE: LazyLock<Regex> =
  LazyLock::new(|| Regex::new(r"\.hmr-(\d+)(\..+)$").expect("invalid hmr edit filename regex"));

fn extract_hmr_step_from_hmr_edit_filename(hmr_filename: &Path) -> usize {
  HMR_EDIT_FILENAME_RE
    .captures(hmr_filename.to_str().unwrap())
    .expect("invalid hmr filename")
    .get(1)
    .unwrap()
    .as_str()
    .parse::<usize>()
    .unwrap()
}

fn get_filename_without_hmr_step(hmr_filename: &Path) -> PathBuf {
  let hmr_filename = hmr_filename.to_str().unwrap();
  let captures = HMR_EDIT_FILENAME_RE.captures(hmr_filename).expect("invalid hmr edit filename");

  let name = &hmr_filename[0..captures.get(0).unwrap().start()];
  let ext = &hmr_filename[captures.get(2).unwrap().start()..];
  let filename = name.to_owned() + ext;
  PathBuf::from(filename)
}

pub fn collect_hmr_edit_files(
  test_folder_path: &Path,
  hmr_temp_dir_path: &Path,
) -> Vec<Vec<PathBuf>> {
  let hmr_files = glob::glob(&format!("{}/**/*.hmr-*.*", test_folder_path.to_str().unwrap()))
    .unwrap()
    .map(|entry| entry.unwrap())
    .filter(|entry| {
      !entry.starts_with(hmr_temp_dir_path)
        && HMR_EDIT_FILENAME_RE.is_match(entry.to_str().unwrap())
    })
    .collect::<Vec<_>>();
  let max_step = hmr_files.iter().fold(None, |max, entry| {
    let value = extract_hmr_step_from_hmr_edit_filename(entry);
    Some(max.map_or(value, |max| value.max(max)))
  });
  let Some(max_step) = max_step else {
    return vec![];
  };
  let mut hmr_files_vec = vec![vec![]; max_step + 1];
  for entry in hmr_files {
    let step = extract_hmr_step_from_hmr_edit_filename(&entry);
    hmr_files_vec[step].push(entry);
  }
  hmr_files_vec
}

pub fn copy_non_hmr_edit_files_to_hmr_temp_dir(test_folder_path: &Path, hmr_temp_dir_path: &Path) {
  let files = glob::glob(&format!("{}/**/*", test_folder_path.to_str().unwrap()))
    .unwrap()
    .map(|entry| entry.unwrap())
    .filter(|entry| {
      !entry.starts_with(hmr_temp_dir_path)
        && !HMR_EDIT_FILENAME_RE.is_match(entry.to_str().unwrap())
        && entry.file_name().is_none_or(|file_name| file_name != "_config.json")
        && entry.is_file()
    })
    .collect::<Vec<_>>();

  for src_path in files {
    let relative = src_path.strip_prefix(test_folder_path).unwrap();
    let dest_path = hmr_temp_dir_path.join(relative);

    if let Some(parent) = dest_path.parent() {
      fs::create_dir_all(parent).unwrap();
    }

    fs::copy(src_path, &dest_path).unwrap();
  }
}

fn get_hmr_edit_file_dest_path(
  test_folder_path: &Path,
  hmr_temp_dir_path: &Path,
  src_path: &Path,
) -> PathBuf {
  let src_file_replaced = get_filename_without_hmr_step(src_path);
  let relative = src_file_replaced.strip_prefix(test_folder_path).unwrap();
  hmr_temp_dir_path.join(relative)
}

pub fn get_changed_files_from_hmr_edit_files(
  test_folder_path: &Path,
  hmr_temp_dir_path: &Path,
  patch: &[PathBuf],
) -> Vec<String> {
  patch
    .iter()
    .map(|src_path| {
      get_hmr_edit_file_dest_path(test_folder_path, hmr_temp_dir_path, src_path)
        .to_str()
        .unwrap()
        .to_owned()
    })
    .collect()
}

pub fn apply_hmr_edit_files_to_hmr_temp_dir(
  test_folder_path: &Path,
  hmr_temp_dir_path: &Path,
  patch: &[PathBuf],
) {
  for src_path in patch {
    let dest_path = get_hmr_edit_file_dest_path(test_folder_path, hmr_temp_dir_path, src_path);

    if let Some(parent) = dest_path.parent() {
      fs::create_dir_all(parent).unwrap();
    }

    fs::copy(src_path, &dest_path).unwrap();
  }
}

#[test]
fn test_extract_hmr_step_from_hmr_edit_filename() {
  assert_eq!(extract_hmr_step_from_hmr_edit_filename(Path::new("foo.hmr-1.js")), 1);
  assert_eq!(extract_hmr_step_from_hmr_edit_filename(Path::new("foo.hmr-1.d.ts")), 1);
}

#[test]
fn test_get_filename_without_hmr_step() {
  assert_eq!(get_filename_without_hmr_step(Path::new("foo.hmr-1.js")), Path::new("foo.js"));
}
