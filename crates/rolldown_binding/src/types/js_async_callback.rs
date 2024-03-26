use napi::{
  bindgen_prelude::{Either3, Promise},
  threadsafe_function::{ThreadsafeFunction, UnknownReturnValue},
};

/// This represents a JavaScript function, whose return value is `Promise<T> | T`
///
/// For example, the equivalent type of `(a: string, b: number) => Promise<number | undefined> | number | undefined` in Rust is
/// `JsAsyncCallback<(String, i32), Option<i32>>`
pub type JsAsyncCallback<Args, Ret> = ThreadsafeFunction<Args, JsAsyncCallbackReturn<Ret>, false>;

// Explicitly using `UnknownReturnValue` in `Either3<Promise<T>, T, UnknownReturnValue>` to get control in rust while
// receiving unknown return value from JS side. This avoids unexpected inner panics.
pub type JsAsyncCallbackReturn<Ret> = Either3<Promise<Ret>, Ret, UnknownReturnValue>;
