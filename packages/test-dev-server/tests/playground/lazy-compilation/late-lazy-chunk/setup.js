// late-lazy-chunk: a lazy chunk rendered before an edit can land after the HMR
// patch for that edit. It must not replace the newer module code. Every file
// here is reached only through `import()`, so the edits do not rebuild the
// pages of the other lazy-compilation specs.
document.getElementById('late-lazy-chunk-a-btn').addEventListener('click', () => {
  import('./route-a.js');
});
document.getElementById('late-lazy-chunk-b-btn').addEventListener('click', async () => {
  const mod = await import('./route-b.js');
  document.getElementById('late-lazy-chunk-b').textContent = mod.b;
});
