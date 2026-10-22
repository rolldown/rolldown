// Local functions and closures.
export function main() {
  const o = { used: 1, dead: 'DEAD_closure' };
  function read() {
    return o.used;
  }
  function flag() {
    return false;
  }
  if (flag()) console.log('DEAD_local_fn');
  return read();
}

// Destructuring reads only the named keys. A rest element that no code reads copies data
// properties only.
const plain = { used: 1, dead: 'DEAD_destructure' };
const { used } = plain;
const nested = { inner: { used: 1, dead: 'DEAD_nested' } };
const {
  inner: { used: nestedUsed },
} = nested;
const renamed = { debug: false, dead: 'DEAD_renamed' };
const { debug: flag } = renamed;
const withRest = { used: 1, dead: 'DEAD_rest' };
const { used: restUsed, ...rest } = withRest;
export function read() {
  if (flag) console.log('DEAD_renamed_branch');
  return used + nestedUsed + restUsed;
}
