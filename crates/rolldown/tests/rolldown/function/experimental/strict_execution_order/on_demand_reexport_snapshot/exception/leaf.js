import { state } from '../state.js';

function capture() {
  if (state === 'before') {
    throw new Error('before');
  }
}
export const SNAPSHOT = /* @__PURE__ */ capture();
