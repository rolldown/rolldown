import { c0 } from './core/c0.js';
import { c1 } from './core/c1.js';
import { c2 } from './core/c2.js';
import { c3 } from './core/c3.js';

globalThis.entriesAwareMain = {
  core: [c0, c1, c2, c3].map((module) => module.name).join(),
  routes: [
    () => import('./routes/r0.js'),
    () => import('./routes/r1.js'),
    () => import('./routes/r2.js'),
    () => import('./routes/r3.js'),
  ],
};
