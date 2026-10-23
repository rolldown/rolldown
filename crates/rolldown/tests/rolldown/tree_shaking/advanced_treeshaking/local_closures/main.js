globalThis.log = [];
const log = (msg) => globalThis.log.push(msg);

// Closures and patterns read the values of local `const` bindings.
export function main() {
  const flag = false;
  function read() {
    return flag;
  }
  if (read()) log('DEAD_closure');
  const obj = { flag: false };
  function readProperty() {
    return obj.flag;
  }
  if (readProperty()) log('DEAD_closure_property');
  const { flag: destructured } = { flag: false };
  if (destructured) log('DEAD_destructure');
  const arrow = () => flag;
  if (arrow()) log('DEAD_arrow');
  return 1;
}

// A call after the declaration of a hoisted function.
export function hoisted() {
  const flag = false;
  if (read()) log('DEAD_hoisted');
  function read() {
    return flag;
  }
  return 1;
}
