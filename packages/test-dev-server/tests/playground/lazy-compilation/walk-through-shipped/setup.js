// walk-through-shipped: a patch can carry a module that a tab never ran, and
// the tab's later lazy chunk for that module must still carry its deps. Every
// file here is reached only through `import()`, so an edit of shared.js does
// not rebuild the pages of the other lazy-compilation specs.
document.getElementById('walk-through-shipped-holder-btn').addEventListener('click', () => {
  import('./holder.js');
});

const routeBtn = document.getElementById('walk-through-shipped-route-btn');
routeBtn.addEventListener('click', async () => {
  const mod = await import('./route.js');
  document.getElementById('walk-through-shipped-route').textContent = mod.route;
});
