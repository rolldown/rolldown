import { state } from '../state.js';

function capture() {
  return state;
}
export const SNAPSHOT = /* @__PURE__ */ capture();
