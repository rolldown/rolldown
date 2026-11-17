import { direct as alias, onlyCatch as implicitAlias } from './self.js';

export var direct = 1;
direct++;
export const results = [];
try {
  throw 0;
} catch (direct) {
  var direct = 3;
  results.push(alias, direct);
}

export { onlyCatch };
try {
  throw 0;
} catch (onlyCatch) {
  for (var onlyCatch of [4]) results.push(implicitAlias, onlyCatch);
}
results.push(onlyCatch);
