// Dynamically loaded module that also uses shared code
import { helper, utilityClass } from './shared.js';
export const value = new utilityClass(helper());
