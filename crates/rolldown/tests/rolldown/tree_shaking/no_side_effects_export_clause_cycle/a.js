import { hoisted, local, reexported } from './main.js';

hoisted();
try {
  local();
} catch {
  globalThis.threw.push('local');
}
try {
  reexported();
} catch {
  globalThis.threw.push('reexported');
}
