{
  const path = './foo.js';
  window.foo // to prevent inlining `path`
  import(path);
}
