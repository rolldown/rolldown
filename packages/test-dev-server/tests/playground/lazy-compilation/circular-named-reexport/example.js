import { publicValue } from './entry.js';

// Runs while `entry.js` is still initializing, so it reads `publicValue`
// through the getter `entry.js` registered before its own body ran.
export const exampleResult = publicValue();
