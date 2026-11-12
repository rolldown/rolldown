use std::{
  fs,
  path::{Path, PathBuf},
  sync::Arc,
  time::{Duration, SystemTime},
};

use crate::{CacheStore, Key, StoreOptions, format};

fn open(dir: &Path) -> CacheStore {
  CacheStore::open(StoreOptions { dir: dir.to_path_buf(), max_age: None })
}

fn list(paths: &[&str]) -> Arc<[String]> {
  paths.iter().map(|p| (*p).to_string()).collect()
}

fn segment_files(dir: &Path) -> Vec<PathBuf> {
  let mut files: Vec<PathBuf> = fs::read_dir(dir)
    .unwrap()
    .map(|e| e.unwrap().path())
    .filter(|p| {
      let name = p.file_name().unwrap().to_str().unwrap();
      name.starts_with("seg-") && name.ends_with(".bin")
    })
    .collect();
  files.sort();
  files
}

/// Changes when the file is rewritten, even under the same name.
fn file_identity(path: &Path) -> (SystemTime, u64) {
  let meta = fs::metadata(path).unwrap();
  #[cfg(unix)]
  let inode = std::os::unix::fs::MetadataExt::ino(&meta);
  #[cfg(not(unix))]
  let inode = 0;
  (meta.modified().unwrap(), inode)
}

fn open_with_max_age(dir: &Path, max_age_ms: u64) -> CacheStore {
  CacheStore::open(StoreOptions {
    dir: dir.to_path_buf(),
    max_age: Some(Duration::from_millis(max_age_ms)),
  })
}

fn value(n: u128) -> Vec<u8> {
  format!("value-{n}").repeat(3).into_bytes()
}

#[test]
fn store_is_send_and_sync() {
  fn check<T: Send + Sync>() {}
  check::<CacheStore>();
}

#[test]
fn open_missing_dir_is_empty_and_does_not_create_it() {
  let tmp = tempfile::tempdir().unwrap();
  let dir = tmp.path().join("cache");
  let store = open(&dir);
  assert!(store.get(1, 2).is_none());
  assert_eq!(store.dependency_lists(1).len(), 0);
  assert!(!store.has_pending());
  store.flush().unwrap();
  assert!(!dir.exists());
}

#[test]
fn round_trip_in_one_process_and_after_reopen() {
  let tmp = tempfile::tempdir().unwrap();
  let dir = tmp.path().join("cache");
  let store = open(&dir);
  store.put(1, list(&["/a.ts", "/tsconfig.json"]), 10, value(1));
  store.put(2, list(&[]), 20, Vec::new());
  assert!(store.has_pending());
  assert_eq!(store.get(1, 10), Some(value(1)));
  assert_eq!(store.dependency_lists(1), vec![list(&["/a.ts", "/tsconfig.json"])]);

  store.flush().unwrap();
  assert!(!store.has_pending());
  assert_eq!(store.get(1, 10), Some(value(1)));
  assert_eq!(store.get(2, 20), Some(Vec::new()));
  assert_eq!(segment_files(&dir).len(), 1);

  let reopened = open(&dir);
  assert_eq!(reopened.get(1, 10), Some(value(1)));
  assert_eq!(reopened.get(2, 20), Some(Vec::new()));
  assert_eq!(reopened.get(1, 11), None);
  assert_eq!(reopened.dependency_lists(1), vec![list(&["/a.ts", "/tsconfig.json"])]);
  assert_eq!(reopened.dependency_lists(2), vec![list(&[])]);
}

#[test]
fn dependency_lists_are_newest_first_across_segments() {
  let tmp = tempfile::tempdir().unwrap();
  let store = open(tmp.path());
  store.put(1, list(&["/one"]), 10, value(1));
  store.flush().unwrap();
  store.put(1, list(&["/two"]), 20, value(2));
  store.flush().unwrap();

  let reopened = open(tmp.path());
  assert_eq!(reopened.dependency_lists(1), vec![list(&["/two"]), list(&["/one"])]);
  reopened.put(1, list(&["/three"]), 30, value(3));
  assert_eq!(
    reopened.dependency_lists(1),
    vec![list(&["/three"]), list(&["/two"]), list(&["/one"])]
  );
}

#[test]
fn identical_lists_and_entries_are_stored_once() {
  let tmp = tempfile::tempdir().unwrap();
  let store = open(tmp.path());
  store.put(1, list(&["/a"]), 10, value(1));
  store.put(1, list(&["/a"]), 10, value(1));
  store.put(1, list(&["/a"]), 11, value(2));
  assert_eq!(store.dependency_lists(1).len(), 1);
  store.flush().unwrap();

  let reopened = open(tmp.path());
  reopened.put(1, list(&["/a"]), 10, value(1));
  assert!(!reopened.has_pending());
  assert_eq!(reopened.dependency_lists(1), vec![list(&["/a"])]);
  assert_eq!(reopened.get(1, 10), Some(value(1)));
  assert_eq!(reopened.get(1, 11), Some(value(2)));
}

#[test]
fn damaged_value_is_not_found() {
  let tmp = tempfile::tempdir().unwrap();
  let store = open(tmp.path());
  store.put(1, list(&["/a"]), 10, value(1));
  store.flush().unwrap();

  let path = &segment_files(tmp.path())[0];
  let mut bytes = fs::read(path).unwrap();
  bytes[format::HEADER_LEN] ^= 0xff;
  fs::write(path, bytes).unwrap();

  let reopened = open(tmp.path());
  assert_eq!(reopened.dependency_lists(1), vec![list(&["/a"])]);
  assert_eq!(reopened.get(1, 10), None);
  // The damaged entry can be saved again.
  reopened.put(1, list(&["/a"]), 10, value(1));
  assert!(reopened.has_pending());
  assert_eq!(reopened.get(1, 10), Some(value(1)));
}

#[test]
fn truncated_segment_is_ignored_and_others_still_work() {
  let tmp = tempfile::tempdir().unwrap();
  let store = open(tmp.path());
  store.put(1, list(&["/a"]), 10, value(1));
  store.flush().unwrap();
  let first = segment_files(tmp.path())[0].clone();
  store.put(2, list(&["/b"]), 20, value(2));
  store.flush().unwrap();

  let file = fs::OpenOptions::new().write(true).open(&first).unwrap();
  let len = file.metadata().unwrap().len();
  file.set_len(len - 5).unwrap();
  drop(file);

  let reopened = open(tmp.path());
  assert_eq!(reopened.get(1, 10), None);
  assert_eq!(reopened.dependency_lists(1).len(), 0);
  assert_eq!(reopened.get(2, 20), Some(value(2)));
  assert_eq!(reopened.dependency_lists(2), vec![list(&["/b"])]);
}

#[test]
fn garbage_and_temp_files_are_ignored() {
  let tmp = tempfile::tempdir().unwrap();
  let dir = tmp.path();
  fs::write(dir.join("seg-garbage.bin"), b"not a segment at all, just some bytes").unwrap();
  fs::write(dir.join("seg-empty.bin"), b"").unwrap();
  fs::write(dir.join("notes.txt"), b"hello").unwrap();
  fs::write(dir.join(".seg-young.tmp"), b"partial").unwrap();
  let old_temp = dir.join(".seg-old.tmp");
  fs::write(&old_temp, b"partial").unwrap();
  fs::File::options()
    .write(true)
    .open(&old_temp)
    .unwrap()
    .set_modified(SystemTime::now() - Duration::from_hours(2))
    .unwrap();

  let store = open(dir);
  assert!(store.get(1, 10).is_none());
  store.put(1, list(&["/a"]), 10, value(1));
  store.flush().unwrap();

  assert!(dir.join(".seg-young.tmp").exists());
  assert!(!old_temp.exists());
  assert!(dir.join("seg-garbage.bin").exists());
  let reopened = open(dir);
  assert_eq!(reopened.get(1, 10), Some(value(1)));
}

#[test]
fn format_version_mismatch_is_ignored() {
  let tmp = tempfile::tempdir().unwrap();
  let store = open(tmp.path());
  store.put(1, list(&["/a"]), 10, value(1));
  store.flush().unwrap();

  let path = &segment_files(tmp.path())[0];
  let mut bytes = fs::read(path).unwrap();
  let version_at = bytes.len() - 16;
  bytes[version_at..version_at + 4].copy_from_slice(&(format::FORMAT_VERSION + 1).to_le_bytes());
  fs::write(path, bytes).unwrap();

  let reopened = open(tmp.path());
  assert_eq!(reopened.get(1, 10), None);
  assert_eq!(reopened.dependency_lists(1).len(), 0);
}

#[test]
fn two_stores_flush_concurrently() {
  let tmp = tempfile::tempdir().unwrap();
  let dir = tmp.path().join("cache");
  std::thread::scope(|scope| {
    for n in 0..8u128 {
      let dir = &dir;
      scope.spawn(move || {
        let store = open(dir);
        for round in 0..4u128 {
          let key1 = n * 100 + round;
          store.put(key1, list(&["/shared", &format!("/{key1}")]), key1, value(key1));
          store.flush().unwrap();
        }
      });
    }
  });

  let reopened = open(&dir);
  for n in 0..8u128 {
    for round in 0..4u128 {
      let key1 = n * 100 + round;
      assert_eq!(reopened.get(key1, key1), Some(value(key1)), "key {key1}");
      assert_eq!(reopened.dependency_lists(key1).len(), 1);
    }
  }
}

#[test]
fn shared_store_is_used_from_many_threads() {
  let tmp = tempfile::tempdir().unwrap();
  let store = open(tmp.path());
  std::thread::scope(|scope| {
    for n in 0..8u128 {
      let store = &store;
      scope.spawn(move || {
        for i in 0..50u128 {
          let key = n * 1000 + i;
          store.put(key, list(&["/x"]), key, value(key));
          assert_eq!(store.get(key, key), Some(value(key)));
          if i % 10 == 0 {
            store.flush().unwrap();
          }
        }
      });
    }
  });
  store.flush().unwrap();
  let reopened = open(tmp.path());
  for n in 0..8u128 {
    for i in 0..50u128 {
      let key = n * 1000 + i;
      assert_eq!(reopened.get(key, key), Some(value(key)));
    }
  }
}

#[cfg(unix)]
#[test]
fn flush_into_read_only_dir_fails_without_panic() {
  use std::os::unix::fs::PermissionsExt;
  let tmp = tempfile::tempdir().unwrap();
  let dir = tmp.path().join("cache");
  fs::create_dir(&dir).unwrap();
  fs::set_permissions(&dir, fs::Permissions::from_mode(0o555)).unwrap();
  // Root can write into a read-only dir.
  if fs::write(dir.join("probe"), b"").is_ok() {
    return;
  }

  let store = open(&dir);
  store.put(1, list(&["/a"]), 10, value(1));
  assert!(store.flush().is_err());
  // The failed batch is dropped, not kept for the next flush.
  assert!(!store.has_pending());
  assert_eq!(store.get(1, 10), None);
  assert!(fs::read_dir(&dir).unwrap().next().is_none());

  store.put(2, list(&["/b"]), 20, value(2));
  assert_eq!(store.get(2, 20), Some(value(2)));
  fs::set_permissions(&dir, fs::Permissions::from_mode(0o755)).unwrap();
  store.flush().unwrap();

  let reopened = open(&dir);
  assert_eq!(reopened.get(1, 10), None);
  assert_eq!(reopened.dependency_lists(1).len(), 0);
  assert_eq!(reopened.get(2, 20), Some(value(2)));
}

#[test]
fn segment_removed_after_open_is_not_found() {
  let tmp = tempfile::tempdir().unwrap();
  let store = open(tmp.path());
  store.put(1, list(&["/a"]), 10, value(1));
  store.flush().unwrap();

  let reopened = open(tmp.path());
  for path in segment_files(tmp.path()) {
    fs::remove_file(path).unwrap();
  }
  assert_eq!(reopened.get(1, 10), None);
  reopened.refresh();
  assert_eq!(reopened.dependency_lists(1).len(), 0);
}

#[test]
fn get_follows_a_merge_by_another_store() {
  let tmp = tempfile::tempdir().unwrap();
  let writer = open(tmp.path());
  for n in 0..10u128 {
    writer.put(n, list(&["/a"]), n, value(n));
    writer.flush().unwrap();
  }
  let reader = open(tmp.path());
  for n in 10..20u128 {
    writer.put(n, list(&["/a"]), n, value(n));
    writer.flush().unwrap();
  }
  // `writer` has merged and removed the segments that `reader` loaded.
  assert!(segment_files(tmp.path()).len() <= 16);
  assert_eq!(reader.get(3, 3), Some(value(3)));
}

#[test]
fn merge_keeps_all_entries_and_lowers_file_count() {
  let tmp = tempfile::tempdir().unwrap();
  let store = open(tmp.path());
  for n in 0..16u128 {
    store.put(n % 4, list(&[&format!("/{n}")]), n, value(n));
    store.flush().unwrap();
  }
  assert_eq!(segment_files(tmp.path()).len(), 16);
  store.put(100, list(&["/x"]), 100, value(100));
  store.flush().unwrap();
  assert_eq!(segment_files(tmp.path()).len(), 1);

  for reader in [&store, &open(tmp.path())] {
    for n in 0..16u128 {
      assert_eq!(reader.get(n % 4, n), Some(value(n)));
    }
    assert_eq!(reader.get(100, 100), Some(value(100)));
    // Newest first is kept by the merge.
    assert_eq!(
      reader.dependency_lists(1),
      vec![list(&["/13"]), list(&["/9"]), list(&["/5"]), list(&["/1"])]
    );
  }
}

#[test]
fn merge_does_not_rewrite_a_large_segment() {
  let tmp = tempfile::tempdir().unwrap();
  let store = open(tmp.path());
  let big_value = vec![7; 5 * 1024 * 1024];
  store.put(1, list(&["/big"]), 1, big_value.clone());
  store.flush().unwrap();
  let big = segment_files(tmp.path())[0].clone();
  let big_identity = file_identity(&big);

  for n in 0..16u128 {
    store.put(1, list(&[&format!("/{n}")]), 100 + n, value(n));
    store.flush().unwrap();
  }
  assert_eq!(segment_files(tmp.path()).len(), 17);
  store.put(1, list(&["/16"]), 116, value(16));
  store.flush().unwrap();

  let files = segment_files(tmp.path());
  assert_eq!(files.len(), 2);
  assert!(files.contains(&big));
  assert_eq!(file_identity(&big), big_identity);

  let mut expected_lists: Vec<_> = (0..=16).rev().map(|n| list(&[&format!("/{n}")])).collect();
  expected_lists.push(list(&["/big"]));
  for reader in [&store, &open(tmp.path())] {
    assert_eq!(reader.get(1, 1).as_ref(), Some(&big_value));
    for n in 0..=16u128 {
      assert_eq!(reader.get(1, 100 + n), Some(value(n)));
    }
    assert_eq!(reader.dependency_lists(1), expected_lists);
  }
}

#[test]
fn merge_keeps_the_time_of_marks_on_other_segments() {
  let tmp = tempfile::tempdir().unwrap();
  let store = open_with_max_age(tmp.path(), 10_000);
  store.set_now_ms(10_000);
  store.put(1, list(&["/big"]), 1, vec![7; 5 * 1024 * 1024]);
  store.flush().unwrap();
  let big = segment_files(tmp.path())[0].clone();

  store.set_now_ms(15_000);
  assert!(store.get(1, 1).is_some());
  store.flush().unwrap();
  for n in 1..=16u128 {
    store.set_now_ms(15_000 + 100 * u64::try_from(n).unwrap());
    store.put(100 + n, list(&["/small"]), n, value(n));
    store.flush().unwrap();
  }
  // The 17 small segments, the first one holding the mark, are merged into one.
  assert_eq!(segment_files(tmp.path()).len(), 2);

  // The mark from 15_000 keeps the large segment alive until 25_000, not longer.
  let store = open_with_max_age(tmp.path(), 10_000);
  store.set_now_ms(24_900);
  store.put(200, list(&["/x"]), 200, value(200));
  store.flush().unwrap();
  assert!(big.exists());
  store.set_now_ms(25_100);
  store.put(201, list(&["/y"]), 201, value(201));
  store.flush().unwrap();
  assert!(!big.exists());
}

#[test]
fn max_age_removes_expired_segment_and_keeps_mostly_live_one() {
  let tmp = tempfile::tempdir().unwrap();
  let first = open_with_max_age(tmp.path(), 1000);
  first.set_now_ms(10_000);
  first.put(1, list(&["/a"]), 10, value(1));
  first.flush().unwrap();
  let expired = segment_files(tmp.path())[0].clone();
  first.put(2, list(&["/b"]), 20, vec![2; 1000]);
  first.put(3, list(&["/c"]), 30, value(3));
  first.flush().unwrap();
  let mostly_live = segment_files(tmp.path()).into_iter().find(|f| *f != expired).unwrap();
  let mostly_live_identity = file_identity(&mostly_live);

  let second = open_with_max_age(tmp.path(), 1000);
  second.set_now_ms(10_900);
  assert_eq!(second.get(2, 20), Some(vec![2; 1000]));
  second.flush().unwrap();
  assert_eq!(segment_files(tmp.path()).len(), 3);

  let third = open_with_max_age(tmp.path(), 1000);
  third.set_now_ms(11_500);
  third.put(4, list(&["/d"]), 40, value(4));
  third.flush().unwrap();

  let files = segment_files(tmp.path());
  assert_eq!(files.len(), 3);
  assert!(!files.contains(&expired));
  assert_eq!(file_identity(&mostly_live), mostly_live_identity);

  let reopened = open(tmp.path());
  assert_eq!(reopened.get(1, 10), None);
  assert_eq!(reopened.dependency_lists(1).len(), 0);
  assert_eq!(reopened.get(2, 20), Some(vec![2; 1000]));
  assert_eq!(reopened.get(4, 40), Some(value(4)));
}

#[test]
fn max_age_removes_unused_entries_and_keeps_used_ones() {
  let tmp = tempfile::tempdir().unwrap();
  let open_with_age = || open_with_max_age(tmp.path(), 1000);

  let first = open_with_age();
  first.set_now_ms(10_000);
  first.put(1, list(&["/used"]), 10, value(1));
  first.put(2, list(&["/unused"]), 20, value(2));
  first.flush().unwrap();
  let half_expired = segment_files(tmp.path())[0].clone();

  let second = open_with_age();
  second.set_now_ms(10_600);
  assert!(!second.has_pending());
  assert_eq!(second.dependency_lists(1), vec![list(&["/used"])]);
  assert_eq!(second.get(1, 10), Some(value(1)));
  assert!(second.has_pending());
  second.flush().unwrap();
  assert_eq!(segment_files(tmp.path()).len(), 2);

  // Half of the value bytes of the first segment are expired, so it is rewritten. The segment
  // with the marks of `second` stays until the marks expire.
  let third = open_with_age();
  third.set_now_ms(11_200);
  third.put(3, list(&["/new"]), 30, value(3));
  third.flush().unwrap();
  let files = segment_files(tmp.path());
  assert_eq!(files.len(), 3);
  assert!(!files.contains(&half_expired));

  let reopened = open_with_age();
  assert_eq!(reopened.get(1, 10), Some(value(1)));
  assert_eq!(reopened.dependency_lists(1), vec![list(&["/used"])]);
  assert_eq!(reopened.get(2, 20), None);
  assert_eq!(reopened.dependency_lists(2).len(), 0);
  assert_eq!(reopened.get(3, 30), Some(value(3)));

  // Without max_age nothing is removed and no usage is recorded.
  let no_age = open(tmp.path());
  no_age.set_now_ms(1_000_000);
  assert_eq!(no_age.get(1, 10), Some(value(1)));
  assert!(!no_age.has_pending());
}

/// Random truncations and bit flips must never cause a panic or a wrong value.
#[test]
fn damaged_bytes_never_panic_or_give_wrong_values() {
  let tmp = tempfile::tempdir().unwrap();
  let source = tmp.path().join("source");
  let store = open(&source);
  let keys: Vec<(Key, Key)> = (0..6u128).map(|n| (n % 3, n)).collect();
  for &(key1, key2) in &keys {
    store.put(key1, list(&["/a", &format!("/{key2}")]), key2, value(key2));
  }
  store.flush().unwrap();
  let original = fs::read(&segment_files(&source)[0]).unwrap();

  let mut seed: u64 = 0x9e37_79b9_7f4a_7c15;
  let mut next = move || {
    seed ^= seed << 13;
    seed ^= seed >> 7;
    seed ^= seed << 17;
    seed
  };
  let len = original.len() as u64;
  for round in 0..400 {
    let mut bytes = original.clone();
    if round % 2 == 0 {
      bytes.truncate(usize::try_from(next() % len).unwrap());
    } else {
      for _ in 0..=(next() % 3) {
        let at = usize::try_from(next() % len).unwrap();
        bytes[at] ^= 1 << (next() % 8);
      }
    }
    let dir = tmp.path().join(format!("case-{round}"));
    fs::create_dir(&dir).unwrap();
    fs::write(dir.join("seg-case.bin"), &bytes).unwrap();

    let damaged = open(&dir);
    for &(key1, key2) in &keys {
      if let Some(found) = damaged.get(key1, key2) {
        assert_eq!(found, value(key2));
      }
      for paths in damaged.dependency_lists(key1) {
        assert!(paths.iter().all(|p| p.starts_with('/')));
      }
    }
  }
}
