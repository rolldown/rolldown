globalThis.log = [];
const log = (msg) => globalThis.log.push(msg);

try {
  new Map([1]);
} catch {
  log('LIVE_map_entry');
}
try {
  new WeakSet([1]);
} catch {
  log('LIVE_weak_set');
}
try {
  Object.keys(null);
} catch {
  log('LIVE_keys_null');
}
try {
  new Uint8Array(-1);
} catch {
  log('LIVE_typed_array_length');
}
try {
  [1].forEach(() => {
    new Map([1]);
  });
} catch {
  log('LIVE_callback');
}
try {
  if (log) {
    new Map([1]);
  }
} catch {
  log('LIVE_block');
}
function unused(x) {
  return 1;
}
try {
  unused(new Map([1]));
} catch {
  log('LIVE_argument');
}
try {
  const object = { value: new Map([1]) };
} catch {
  log('LIVE_property');
}
try {
  log ? new Map([1]) : 0;
} catch {
  log('LIVE_conditional');
}
try {
  log && new Map([1]);
} catch {
  log('LIVE_logical');
}
function inFunction() {
  try {
    new Map([1]);
  } catch {
    log('LIVE_function');
  }
}
inFunction();
const settings = { off: false };
try {
  if (settings.off) log('never');
  new WeakMap([[1, 1]]);
} catch {
  log('LIVE_after_folded_branch');
}

export const done = true;
