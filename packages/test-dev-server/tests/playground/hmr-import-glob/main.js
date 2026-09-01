// Three globs, each covering a different part of the feature:
// - `pages` is eager, so a new match has to reach the module graph and execute, not just show up as
//   a key.
// - `nested` is lazy over a `**` pattern, where a match can appear in a directory that did not exist
//   when the glob was first walked.
// - `later` points at a directory that does not exist at boot, so nothing under it can be watched
//   until the directory itself shows up.
const pages = import.meta.glob('./pages/*.js', { eager: true });
const nested = import.meta.glob('./nested/**/*.js');
const later = import.meta.glob('./later/*.js');

window.__globRuns = (window.__globRuns ?? 0) + 1;

const keys = (modules) => Object.keys(modules).sort().join(',');

document.querySelector('.pages').textContent = keys(pages);
document.querySelector('.titles').textContent = Object.keys(pages)
  .sort()
  .map((key) => pages[key].title)
  .join(',');
document.querySelector('.nested').textContent = keys(nested);
document.querySelector('.later').textContent = keys(later);
document.querySelector('.runs').textContent = `runs:${window.__globRuns}`;

import.meta.hot.accept();
