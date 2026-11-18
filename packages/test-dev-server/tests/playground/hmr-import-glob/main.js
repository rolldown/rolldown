// `pages` is eager, so a new match has to execute, not only show up as a key.
// `later/` does not exist at boot. For `deep`, its parent `missing/` does not exist either.
const pages = import.meta.glob('./pages/*.js', { eager: true });
const nested = import.meta.glob('./nested/**/*.js');
const later = import.meta.glob('./later/*.js');
const deep = import.meta.glob('./missing/deep/*.js');

window.__globRuns = (window.__globRuns ?? 0) + 1;

const keys = (modules) => Object.keys(modules).sort().join(',');

document.querySelector('.pages').textContent = keys(pages);
document.querySelector('.titles').textContent = Object.keys(pages)
  .sort()
  .map((key) => pages[key].title)
  .join(',');
document.querySelector('.nested').textContent = keys(nested);
document.querySelector('.later').textContent = keys(later);
document.querySelector('.deep').textContent = keys(deep);

import.meta.hot.accept();
