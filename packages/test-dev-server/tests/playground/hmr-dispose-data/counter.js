// `dispose` runs before this module is replaced and stashes state on `import.meta.hot.data`,
// which persists across updates; the re-executed module reads it back.
const prev = import.meta.hot?.data?.saved ?? 'none';

export const value = 'dispose-v1';
document.querySelector('.value').textContent = value;
document.querySelector('.prev').textContent = prev;

import.meta.hot?.accept();
import.meta.hot?.dispose((data) => {
  data.saved = value;
});
