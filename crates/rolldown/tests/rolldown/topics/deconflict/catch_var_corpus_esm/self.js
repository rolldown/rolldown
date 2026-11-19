// Self-imports of a module-level linked var read inside the catch.
import { s as alias, only as onlyAlias } from './self.js';
import * as self from './self.js';

export var s = 1;
s++;
export const selfLog = [];
try {
  throw 0;
} catch (s) {
  var s = 3;
  selfLog.push(s, alias, self.s);
}

export { only };
try {
  throw 0;
} catch (only) {
  var only = 4;
  selfLog.push(only, onlyAlias);
}
selfLog.push(only);
