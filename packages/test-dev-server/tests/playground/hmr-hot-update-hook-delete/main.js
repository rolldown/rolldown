import './child-a.js';
import './child-b.js';

window.__mainRuns = (window.__mainRuns ?? 0) + 1;
document.querySelector('.value').textContent = `runs:${window.__mainRuns}`;

import.meta.hot.accept(() => {});
