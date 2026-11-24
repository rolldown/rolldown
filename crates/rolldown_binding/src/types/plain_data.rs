/// Whether the binding gives JS plain data instead of objects that keep native data alive.
///
/// The GC of the JS engine does not see Wasm memory, so it can take very long to collect an object
/// whose data is in Wasm memory. Plain data lives in the JS heap, where the GC sees it.
pub const PLAIN_DATA: bool = cfg!(target_family = "wasm");
