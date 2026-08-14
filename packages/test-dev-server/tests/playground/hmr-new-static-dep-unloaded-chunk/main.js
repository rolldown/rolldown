import './hmr.js';
import { load } from './lazy-holder.js';

// The lazy chunk compiles (and enters the server's module graph) only when a
// tab clicks the button. A tab that never clicks holds neither heavy.js's
// module nor its factory.
document.querySelector('.load-heavy').addEventListener('click', async () => {
  const mod = await load();
  document.querySelector('.heavy').textContent = mod.heavy;
});
