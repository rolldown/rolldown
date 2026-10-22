globalThis.log = [];
const log = (msg) => globalThis.log.push(msg);

// Calls with different arguments.
function mixed(x) {
  if (x) log('LIVE_mixed');
}
mixed(false);
mixed(true);

// Unknown code can call an exported function.
export function escaped(x) {
  if (x) log('LIVE_escaped');
}
escaped(false);

// The analysis does not record a call through an unknown callee, so both functions escape.
function viaA(x) {
  if (x) log('LIVE_viaA');
}
function viaB(x) {
  if (x) log('LIVE_viaB');
}
viaA(false);
viaB(false);
(globalThis.pickA ? viaA : viaB)(true);

// A write to the parameter.
function reassigned(x) {
  x = true;
  if (x) log('LIVE_reassigned');
}
reassigned(false);

// A write through a parameter changes the object of the caller.
const cfg = { debug: false };
function enable(o) {
  o.debug = true;
}
enable(cfg);
if (cfg.debug) log('LIVE_param_write');

// A write through a local alias.
const cfg2 = { debug: false };
function enable2() {
  const alias = cfg2;
  alias.debug = true;
}
enable2();
if (cfg2.debug) log('LIVE_alias_write');

// A write through a returned parameter.
const cfg3 = { debug: false };
function id(o) {
  return o;
}
id(cfg3).debug = true;
if (cfg3.debug) log('LIVE_return_write');

// A write deopt applies to a call that the analysis records later.
const cfg4 = { flag: false };
function setLater(o) {
  o.flag = true;
}
setLater({});
setLater(cfg4);
if (cfg4.flag) log('LIVE_later_call');

// Unknown code calls a callback.
function callback(x) {
  if (x) log('LIVE_callback');
}
callback(false);
[true].forEach(callback);

// `call` calls the function with arguments that the analysis does not record.
function viaCall(x) {
  if (x) log('LIVE_call');
}
viaCall(false);
viaCall.call(null, true);

// A default value.
function withDefault(x = true) {
  if (x) log('LIVE_default');
}
withDefault(undefined);

// A spread argument.
function spread(x) {
  if (x) log('LIVE_spread');
}
spread(false);
spread(...[true]);

// `arguments` makes the arguments escape.
function leak(o) {
  return arguments[0];
}
log(leak({ a: 'LIVE_arguments', b: 1 }).a);

// Recursion with a changing argument.
function countdown(n) {
  if (n === 1) log('LIVE_recursion');
  if (n > 0) countdown(n - 1);
}
countdown(2);

// An object without a holder that code reads after a write.
function setV(o) {
  o.v = 'LIVE_holderless';
  return o;
}
log(setV({}).v);

// A getter of an argument runs.
function readX(o) {
  return o.x;
}
readX({
  get x() {
    log('LIVE_getter');
    return 1;
  },
});

// A destructured parameter runs a getter of the argument.
function destructure({ y }) {
  return y;
}
destructure({
  get y() {
    log('LIVE_destructure_getter');
    return 1;
  },
});

// Unknown code reads what an escaped call returns.
function make() {
  return { a: 'LIVE_return_escape', b: 1 };
}
globalThis.made = make();

// A function declaration inside a function that code writes.
function localFn() {
  function f() {
    return false;
  }
  f = () => true;
  if (f()) log('LIVE_local_fn');
}
localFn();

// Each closure sees the parameter of the call that made it.
function capture(x) {
  return () => x;
}
const capturedTrue = capture(true);
const capturedFalse = capture(false);
if (capturedTrue() && !capturedFalse()) log('LIVE_closure_param');

export const done = true;
