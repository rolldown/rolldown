fn main() {
  use napi_build::setup;
  println!("cargo::rustc-check-cfg=cfg(tokio_unstable)");
  setup();
}
