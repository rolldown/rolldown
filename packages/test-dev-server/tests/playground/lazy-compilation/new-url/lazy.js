// Lazily compiled on first click. The asset is referenced via
// `new URL(..., import.meta.url)`. In a full build the core finalizer rewrites
// the first argument to the hashed asset URL and emits the asset; the HMR/lazy
// codegen uses a different finalizer that does NOT rewrite it, so the patch
// ships the raw specifier and the browser resolves it against the patch URL —
// fetching the wrong path and 404ing. See rolldown#9812 / vitejs/vite#22596.
const img = document.createElement('img');
img.id = 'new-url-image';
img.src = new URL('./new-url-image.png', import.meta.url).href;
document.getElementById('new-url-container').appendChild(img);
