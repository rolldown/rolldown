import { setIdentity } from './lib.js';

setIdentity((value) => {
  globalThis.calls.push('live binding');
  return value;
});
