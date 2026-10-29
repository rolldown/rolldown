// top-level-await: the lazy module uses top-level await. Its lazy chunk wraps
// the module body in a `registerFactory` function, which must be `async` or
// the chunk fails to parse.
document.getElementById('top-level-await-btn').addEventListener('click', () => {
  import('./tla.js');
});

// The runtime does not await that `async` factory, so an importer reads the
// exports before the await finishes.
document.getElementById('top-level-await-tdz-btn').addEventListener('click', () => {
  import('./tla-importer.js');
});
