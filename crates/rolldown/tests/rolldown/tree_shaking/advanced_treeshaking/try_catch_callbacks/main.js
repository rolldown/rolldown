globalThis.log = [];
const log = (msg) => globalThis.log.push(msg);

const attempt = (run) => {
  try {
    return run();
  } catch {
    return 0;
  }
};
const later = (run) => {
  try {
    log.length;
  } catch {}
  return run;
};
const cleanup = (run) => {
  try {
    run();
  } finally {
    log('cleanup');
  }
};
const second = (name, run) => {
  try {
    run();
  } catch {}
};
// The `_preserved` fixtures call their callbacks like this, so that the walk of a callback
// checks the rule of the fixture. When the `try` rule follows aliases, change their helper.
const aliased = (run) => {
  const call = run;
  try {
    call();
  } catch {}
};
const settings = { off: false };
// The option-off build keeps a call of `read`, because the call reads a property.
function read(marker) {
  return settings.off;
}

export function main() {
  const value = attempt(() => {
    if (settings.off) log('DEAD_branch_in_caught_callback');
    return 1;
  });
  later(() => {
    read('DEAD_free_callback');
  })();
  cleanup(() => {
    read('DEAD_finally_callback');
  });
  second(read('DEAD_unused_argument'), () => {});
  aliased(() => {
    read('DEAD_aliased_callback');
  });
  return value;
}
