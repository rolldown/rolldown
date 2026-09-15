import { ns } from './entry.js';

// Runs while `entry.js` is still initializing, so it reads the namespace through
// the getter `entry.js` registered before its own body ran.
export const exampleResult = ns.value();
