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

// The default sort converts each element to a string.
let converted = false;
[
  {
    toString() {
      converted = true;
      return 'a';
    },
  },
  'b',
].sort();
if (converted) log('LIVE_sort_to_string');

// The comparator runs, and the sort converts its result to a number.
let compared = false;
[2, 1].sort((a, b) => {
  compared = true;
  return a - b;
});
if (compared) log('LIVE_sort_comparator');
let resultConverted = false;
[2, 1].sort(() => ({
  valueOf() {
    resultConverted = true;
    return 0;
  },
}));
if (resultConverted) log('LIVE_sort_result');

// `reduce` without a start value throws for an array without elements.
attempt('LIVE_reduce_empty', () => {
  [].reduce((a, b) => a + b);
});
attempt('LIVE_reduce_right_holes', () => {
  [, ,].reduceRight((a, b) => a + b);
});

// The reducer converts the elements, the start value, and what it returned before.
[
  {
    valueOf() {
      log('LIVE_reduce_element');
      return 1;
    },
  },
].reduce((acc, x) => acc + x, 0);
[1].reduce((acc, x) => acc + x, {
  valueOf() {
    log('LIVE_reduce_start');
    return 0;
  },
});
[1, 2].reduce(
  (acc, x) =>
    acc === 0
      ? {
          valueOf() {
            log('LIVE_reduce_accumulator');
            return 1;
          },
        }
      : acc + x,
  0,
);

// A conversion of a numeric argument runs code.
const toIndex = (name) => ({
  valueOf() {
    log(name);
    return 0;
  },
});
[1, 2].splice(toIndex('LIVE_splice_start'), 1);
[1, 2].copyWithin(toIndex('LIVE_copy_within_target'), 1);
[1, 2].fill(0, toIndex('LIVE_fill_start'));

// `concat` reads `Symbol.isConcatSpreadable` of an object argument.
[1].concat({
  get [Symbol.isConcatSpreadable]() {
    log('LIVE_concat_spreadable');
    return false;
  },
});

// The flattening reads the elements of an array that a proxy wraps.
let flatRead = false;
const flatProxy = new Proxy([], {
  get(target, key, receiver) {
    flatRead = true;
    return Reflect.get(target, key, receiver);
  },
});
[flatProxy].flat();
if (flatRead) log('LIVE_flat_proxy');
let flatMapRead = false;
const flatMapProxy = new Proxy([], {
  get(target, key, receiver) {
    flatMapRead = true;
    return Reflect.get(target, key, receiver);
  },
});
[1].flatMap(() => flatMapProxy);
if (flatMapRead) log('LIVE_flat_map_proxy');

// A write can replace a method or set `constructor`.
const ownMethod = [1];
ownMethod.sort = () => log('LIVE_own_sort');
ownMethod.sort();
const species = [1];
species.constructor = {
  [Symbol.species]: function () {
    log('LIVE_concat_species');
    return {};
  },
};
species.concat([2]);

// A later read sees the new order or the new elements.
const reversed = [false, true];
reversed.reverse();
if (reversed[0]) log('LIVE_reverse_read');
const spliced = [false, true];
spliced.splice(0, 1);
if (spliced[0]) log('LIVE_splice_read');
const inserted = [false];
inserted.splice(0, 0, true);
if (inserted[0]) log('LIVE_splice_insert');
const copied = [false, true];
copied.copyWithin(0, 1);
if (copied[0]) log('LIVE_copy_within_read');
const filled = [false];
filled.fill(true);
if (filled[0]) log('LIVE_fill_read');
const sorted = [true, false];
sorted.sort();
if (sorted[1]) log('LIVE_sort_read');

export const done = true;
