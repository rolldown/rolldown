// `isEqual` is read through this module. `isSame` is read straight from its definer, so tree
// shaking drops this module's import of it while the re-export keeps its binding demand.
import { isEqual } from '../shared-equality.js';
import { isSame } from '../shared-identity.js';

export { isEqual, isSame };

export function hasExpectedValue(actual) {
  return actual === 'expected';
}
