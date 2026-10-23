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

attempt('LIVE_radix_low', () => {
  (1).toString(1);
});
attempt('LIVE_radix_high', () => {
  (1).toString(37);
});
attempt('LIVE_to_fixed_range', () => {
  (1).toFixed(101);
});
attempt('LIVE_to_fixed_infinity', () => {
  (1).toFixed(Infinity);
});
attempt('LIVE_to_precision_range', () => {
  (1).toPrecision(0);
});
attempt('LIVE_to_exponential_range', () => {
  (1).toExponential(-1);
});
(1).toString({
  valueOf() {
    log('LIVE_radix_value_of');
    return 10;
  },
});
(1).toFixed({
  valueOf() {
    log('LIVE_digits_value_of');
    return 1;
  },
});
(1).toString(10, log('LIVE_extra_argument'));

export const done = true;
