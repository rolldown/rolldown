fn main() {
  // Current rustc exposes identical cfg sets for wasm32-wasip1 and
  // wasm32-wasip1-threads, so select the writer backend from Cargo's exact
  // target instead.
  println!("cargo::rustc-check-cfg=cfg(rolldown_wasi_threads)");
  if std::env::var("TARGET").as_deref() == Ok("wasm32-wasip1-threads") {
    println!("cargo::rustc-cfg=rolldown_wasi_threads");
  }
}
