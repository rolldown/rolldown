import './cycle-b.js';

export const value = 'cycle-v1';

(window.__cycleRuns ??= []).push(value);
document.querySelector('.cycle').textContent = value;
