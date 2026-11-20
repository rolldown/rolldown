/* @__NO_SIDE_EFFECTS__ */
function local() {
  return 1;
}
function setup() {
  eval("local = () => globalThis.calls.push('local')");
}
setup();
local();
