import { value } from './app.js';

globalThis.__mainRuns = (globalThis.__mainRuns ?? 0) + 1;
document.querySelector('.main-runs').textContent = String(globalThis.__mainRuns);
document.querySelector('.main-saw').textContent = value;
