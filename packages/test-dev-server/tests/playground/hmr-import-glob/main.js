// Each glob covers a different part of the feature:
// - `pages` is eager, so a new match has to reach the module graph and execute, not just show up as
//   a key.
// - `nested` is lazy over a `**` pattern, where a match can appear in a directory that did not exist
//   when the glob was first walked.
// - `later` and `deep` point at directories that do not exist at boot. For `deep`, the parent is
//   missing too.
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
document.querySelector('.runs').textContent = `runs:${window.__globRuns}`;

import.meta.hot.accept();
