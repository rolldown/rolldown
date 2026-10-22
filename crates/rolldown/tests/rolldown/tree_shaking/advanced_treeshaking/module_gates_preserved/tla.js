export let ready = false;
export const state = { ready: false };
let value;
export function late() {
  return value;
}
await null;
ready = true;
state.ready = true;
value = 'set';
