import { defaults } from './a.js';

// `B` is a class; `.name` is 'B'. If the cyclic import is bound correctly the
// page renders `button=B`; if it is emitted unbound the module-init throws
// `ReferenceError: B is not defined` and `.app` stays empty.
document.querySelector('.app').textContent = 'button=' + defaults.button.name;
