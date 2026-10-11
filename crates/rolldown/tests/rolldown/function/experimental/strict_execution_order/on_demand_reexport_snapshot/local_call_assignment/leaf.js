import { state } from '../state.js';

export let SNAPSHOT;
function capture() {
  SNAPSHOT = state;
}
capture();
