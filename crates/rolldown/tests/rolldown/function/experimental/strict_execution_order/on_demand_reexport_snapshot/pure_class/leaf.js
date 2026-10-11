import { state } from '../state.js';

class Capture {
  constructor() {
    this.value = state;
  }
}
export const SNAPSHOT = /* @__PURE__ */ new Capture();
