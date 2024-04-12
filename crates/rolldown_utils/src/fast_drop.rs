use std::{mem, thread};

pub fn fast_drop<T>(src: T)
where
  T: Send + 'static,
{
  thread::spawn(move || {
    mem::drop(src);
  });
}
