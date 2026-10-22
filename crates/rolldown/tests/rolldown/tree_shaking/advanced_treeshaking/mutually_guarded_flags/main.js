let DEAD_hydrating = false,
  DEAD_mismatch = false;
function set_hydrating(h) {
  DEAD_hydrating = h;
}
if (DEAD_hydrating) {
  DEAD_mismatch = true;
}
if (DEAD_mismatch) {
  set_hydrating(true);
}
if (DEAD_hydrating) {
  console.log('DEAD_HYDRATE');
}
export const done = true;
