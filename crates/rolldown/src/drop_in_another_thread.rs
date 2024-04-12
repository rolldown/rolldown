use std::{mem, thread};

use crate::types::{ast_table::AstTable, module_table::ModuleTable, symbols::Symbols};

fn drop_in_another_thread<T>(src: T)
where
  T: Send + 'static,
{
  thread::spawn(move || {
    mem::drop(src);
  });
}

/// Because the `ModuleTable/Symbols/AstTable` used at all stages, we must drop them in the final process.
/// Them has larger memory usage, drop them caused larger overhead.
/// We can drop them in another thread to reduce the overhead, it will not block main thread to improve performance, see https://news.ycombinator.com/item?id=23362518.

/// Here using the pattern of impl `Drop` trait to it,
/// it avoid drop them manually at `scan_stage` or `bundle_stage`.
impl Drop for ModuleTable {
  fn drop(&mut self) {
    drop_in_another_thread(std::mem::take(self));
  }
}

impl Drop for Symbols {
  fn drop(&mut self) {
    drop_in_another_thread(std::mem::take(self));
  }
}

impl Drop for AstTable {
  fn drop(&mut self) {
    drop_in_another_thread(std::mem::take(self));
  }
}
