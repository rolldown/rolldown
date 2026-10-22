// An optional read and an optional call of known values fold branches.
const config = { debug: false };
const runner = {
  run() {
    return false;
  },
};
function check() {
  return true;
}
export function main() {
  if (config?.debug) console.log('DEAD_optional_read');
  if (runner.run?.()) console.log('DEAD_optional_call');
  // A `break` that always runs ends the loop.
  for (;;) {
    if (check()) break;
    console.log('DEAD_after_break');
  }
  return 1;
}

// A `for...in` loop with an empty body over an object literal has no effect.
for (const key in { a: 'DEAD_for_in' }) {
}
