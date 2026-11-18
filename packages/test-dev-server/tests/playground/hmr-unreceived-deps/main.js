import './hmr.js';
import './dynamic-importer.js';

// A tab that clicks compiles lazy-holder.js and its imports into the server's
// module graph; only that tab receives their factories.
document.querySelector('.load').addEventListener('click', async () => {
  const mod = await import('./lazy-holder.js');
  document.querySelector('.lazy').textContent = await mod.load();
});

document.querySelector('.load-late').addEventListener('click', async () => {
  await import('./late-accept.js');
  document.querySelector('.late').textContent = globalThis.__lateAcceptValue;
});
