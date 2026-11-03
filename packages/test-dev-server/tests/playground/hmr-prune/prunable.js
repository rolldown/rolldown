// When `app` stops importing this module, its `prune` callback should fire.
document.querySelector('.prunable').textContent = 'present';

import.meta.hot?.prune(() => {
  document.querySelector('.prunable').textContent = 'pruned';
});
