globalThis.log = [];
const log = (msg) => globalThis.log.push(msg);
// The alias hides the callback from the `try` rule, so the walk of the callback checks it.
const attempt = (name, run) => {
  const call = run;
  try {
    call();
  } catch {
    log(name);
  }
};

// A closure that runs before the declaration throws.
function early() {
  function read() {
    return flag;
  }
  if (read()) log('never');
  const flag = false;
}
attempt('LIVE_tdz_closure', early);

// A hoisted function that runs before the declaration throws.
function hoistedEarly() {
  if (read()) log('never');
  const flag = false;
  function read() {
    return flag;
  }
}
attempt('LIVE_tdz_hoisted', hoistedEarly);

// A callback that runs before the declaration throws.
function viaCallback() {
  const read = () => flag;
  [1].forEach(() => {
    if (read()) log('never');
  });
  const flag = false;
}
attempt('LIVE_tdz_callback', viaCallback);

// `typeof` before the declaration throws.
function typeofEarly() {
  if (typeof flag === 'boolean') log('never');
  const flag = false;
}
attempt('LIVE_tdz_typeof', typeofEarly);

// A `const` in a `case` clause has no value in another clause.
const settings = { off: false };
function clause(kind) {
  switch (kind) {
    case 0:
      const flag = settings.off;
      break;
    default:
      if (flag) log('never');
  }
}
attempt('LIVE_case_clause', () => clause(1));

// A closure can write a `let`.
function written() {
  let flag = false;
  const set = () => {
    flag = true;
  };
  set();
  if (flag) log('LIVE_let_write');
}
written();

// A closure that runs after the declaration reads the value.
function late() {
  const flag = true;
  const read = () => flag;
  if (read()) log('LIVE_closure_value');
}
late();

export const done = true;
