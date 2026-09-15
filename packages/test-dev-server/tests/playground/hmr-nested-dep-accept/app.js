import { value } from './dep.js';
document.querySelector('.nested').textContent = value;

// Accept the direct dep. Editing the transitive `nested.js` bubbles up through `dep.js` to
// this boundary; the callback gets the fresh `dep` (which re-exports the updated value).
import.meta.hot?.accept('./dep.js', (mod) => {
  document.querySelector('.nested').textContent = mod.value;
});
