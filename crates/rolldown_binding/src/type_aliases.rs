use std::sync::Mutex;

use rolldown_utils::unique_arc::{UniqueArc, WeakRef};

pub type UniqueArcMutex<T> = UniqueArc<Mutex<T>>;
pub type WeakRefMutex<T> = WeakRef<Mutex<T>>;
