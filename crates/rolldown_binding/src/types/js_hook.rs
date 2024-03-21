use napi::{
  bindgen_prelude::{Either3, Promise},
  threadsafe_function::{ThreadsafeFunction, UnknownReturnValue},
};

// Using `Either3<Promise<T>, T, UnknownReturnValue>` in callback functions to handle the
// unknown return value from JavaScript explicit and avoid unexpected panics.
pub type JsHook<Args, Ret> = ThreadsafeFunction<Args, JsHookReturn<Ret>, false>;

pub type JsHookReturn<Ret> = Either3<Promise<Ret>, Ret, UnknownReturnValue>;
