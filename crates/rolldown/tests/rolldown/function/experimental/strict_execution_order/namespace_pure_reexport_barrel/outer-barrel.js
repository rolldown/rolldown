// Outer star-re-export barrel (like victory-vendor's `export * from "d3-scale"`). It re-exports the
// inner barrel. Its own namespace getters resolve (canonically) to the *leaf* definer bindings, so
// the leaf definers are its execution dependencies but the intermediate inner barrel is NOT — which
// is exactly why the barrel-forward decision judges the re-export record dead and emits an empty
// `init_*`.
export * from './scale-barrel.js';
