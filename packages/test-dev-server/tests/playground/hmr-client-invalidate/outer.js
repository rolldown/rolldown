import { inner } from './inner.js';

(window.__outerRuns ??= []).push(inner);
document.querySelector('.outer').textContent = inner;

import.meta.hot?.accept();
