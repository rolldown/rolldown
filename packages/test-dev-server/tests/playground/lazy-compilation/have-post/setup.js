// have-post: the spec lowers the runtime's `maxQueryLength` to 0, so every patch and lazy
// request first sends its query in a POST body and carries only the token. Every file here is
// reached only through `import()`, so the edits do not rebuild the pages of the other
// lazy-compilation specs.
document.getElementById('have-post-btn').addEventListener('click', () => {
  import('./route.js');
});
