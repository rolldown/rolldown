// App entry point (like the user's `src/main.js`).
// Uses exports from the `core` package that declares `"sideEffects": false`.
import { setupWorker, http } from './core/index.js';

const worker = setupWorker(http());
console.log(worker);
