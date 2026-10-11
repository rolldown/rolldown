import { state } from '../state.js';

function capture() {
  return state;
}
export let SNAPSHOT;
SNAPSHOT = /* @__PURE__ */ capture();
