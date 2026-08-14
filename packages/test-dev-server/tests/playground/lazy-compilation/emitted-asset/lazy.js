// Lazily compiled on first click. Importing the image emits it as a hashed
// asset (via the dev server's ported asset plugin) and rewrites this import to
// the asset's URL. The browser then fetches that URL, which must already
// resolve and be served by the time this patch reaches it. See vitejs/vite#22596.
import imageUrl from './lazy-image.png';

const img = document.createElement('img');
img.id = 'emitted-asset-image';
img.src = imageUrl;
document.getElementById('emitted-asset-container').appendChild(img);
