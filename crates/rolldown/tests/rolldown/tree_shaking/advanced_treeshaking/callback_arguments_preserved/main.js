globalThis.log = [];
const log = (msg) => globalThis.log.push(msg);

// An element that converts runs code in the callback.
[
  {
    valueOf() {
      log('LIVE_unary_value_of');
      return 1;
    },
  },
].map((x) => +x);
[
  {
    valueOf() {
      log('LIVE_bitwise_value_of');
      return 1;
    },
  },
].map((x) => x | 0);
[
  {
    toString() {
      log('LIVE_template_to_string');
      return 'x';
    },
  },
].map((x) => `${x}`);
let sortConverted = false;
[
  1,
  {
    valueOf() {
      sortConverted = true;
      return 2;
    },
  },
].sort((a, b) => a - b);
if (sortConverted) log('LIVE_sort_value_of');

// A write or `push` changes the elements that the callback gets.
const written = ['a'];
written[0] = {
  toString() {
    log('LIVE_written_element');
    return 'b';
  },
};
written.map((x) => `${x}`);
const pushed = ['a'];
pushed.push({
  valueOf() {
    log('LIVE_pushed_element');
    return 1;
  },
});
pushed.forEach((x) => +x);

// The callback can write the array through its third parameter.
const viaArray = [false];
viaArray.forEach((x, i, all) => {
  all[i] = true;
});
if (viaArray[0]) log('LIVE_array_parameter_write');

// A direct call of the callback adds the values of its arguments.
function convert(x) {
  return +x;
}
convert({
  valueOf() {
    log('LIVE_direct_call');
    return 1;
  },
});
['a'].map(convert);

// The callback gets the `this` argument.
[1].forEach(
  function () {
    if (this.flag) log('LIVE_this_argument');
  },
  { flag: true },
);

// A getter of an element runs.
[
  {
    get value() {
      log('LIVE_element_getter');
      return 1;
    },
  },
].forEach((x) => x.value);

// The index and the accumulator have values that the code compares.
['a', 'b'].forEach((x, i) => {
  if (i === 1) log('LIVE_index_value');
});
const total = [1, 2].reduce((acc, x) => acc + x, 0);
if (total === 3) log('LIVE_reduce_total');

export const done = true;
