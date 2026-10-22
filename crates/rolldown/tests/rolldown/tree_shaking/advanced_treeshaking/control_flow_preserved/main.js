globalThis.log = [];
const log = (msg) => globalThis.log.push(msg);

// A `break` that depends on a value that changes, and a test of a `break` with an effect.
let calls = 0;
function check() {
  calls++;
  return calls > 1;
}
for (;;) {
  if (check()) break;
  log('LIVE_after_break');
}
function checkWithEffect() {
  log('LIVE_check_effect');
  return true;
}
for (;;) {
  if (checkWithEffect()) break;
}

// Labels, `continue`, `switch`, `finally`, and `return` in loops.
outer: for (let i = 0; i < 2; i++) {
  for (;;) {
    if (i === 0) continue outer;
    break outer;
  }
}
log('LIVE_labeled');
for (let i = 0; i < 1; i++) {
  switch (i) {
    case 0:
      break;
  }
  log('LIVE_switch_break');
}
for (;;) {
  try {
    break;
  } finally {
    log('LIVE_break_finally');
  }
}
function returnInLoop() {
  for (const value of [1, 2]) {
    if (value === 2) return 'LIVE_return_in_loop';
  }
  return 'NEVER_after_loop';
}
log(returnInLoop());

// Loop bodies and initializers with effects.
do {
  log('LIVE_do_once');
} while (false);
for (log('LIVE_for_init'); false;) {}
for (const key in { a: 1 }) log(`LIVE_for_in_${key}`);
for (const key in { __proto__: { inherited: 1 } }) log(`LIVE_for_in_${key}`);

// Optional chains on `null`, an optional call after a write, and an optional call with an
// effect.
const nothing = null;
if (nothing?.a.b === undefined) log('LIVE_optional_short_circuit');
const runner = {
  run() {
    return false;
  },
};
runner.run = () => true;
if (runner.run?.()) log('LIVE_optional_call');
const withEffect = {
  run() {
    log('LIVE_optional_effect');
    return false;
  },
};
withEffect.run?.();
const nested = { a: { b: false } };
nested.a = null;
if (nested?.a?.b === undefined) log('LIVE_optional_after_write');

export const done = true;
