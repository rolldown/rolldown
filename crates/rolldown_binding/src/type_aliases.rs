use std::sync::Mutex;

use rolldown_utils::unique_arc::{UniqueArc, WeakRef};
#[allow(dead_code)]
pub type UniqueArcMutex<T> = UniqueArc<Mutex<T>>;
#[allow(dead_code)]
pub type WeakRefMutex<T> = WeakRef<Mutex<T>>;
