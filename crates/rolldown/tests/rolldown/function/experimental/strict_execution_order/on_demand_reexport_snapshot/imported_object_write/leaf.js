import { state, phase } from './state.js';

state.value = phase;

export function readValue() {
  return state.value;
}
