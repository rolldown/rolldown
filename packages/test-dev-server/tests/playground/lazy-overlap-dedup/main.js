// Two lazy "routes" that both statically import shared.js. Loaded via
// buttons so the spec controls the (sequential) navigation order.
document.getElementById('route-a-btn').addEventListener('click', async () => {
  const mod = await import('./page-a.js');
  document.getElementById('route-a-content').textContent = mod.a;
});

document.getElementById('route-b-btn').addEventListener('click', async () => {
  const mod = await import('./page-b.js');
  document.getElementById('route-b-content').textContent = mod.b;
});
