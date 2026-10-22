globalThis.log = [];
const log = (msg) => globalThis.log.push(msg);

// Writes and method calls change the elements and the length.
const written = [false];
written[0] = true;
if (written[0]) log('LIVE_index_write');
const added = [];
added[0] = true;
if (added[0]) log('LIVE_new_index_write');
const pushed = [];
pushed.push('LIVE_push');
log(pushed[0]);
const popped = [true];
popped.pop();
if (popped[0] === undefined) log('LIVE_pop');
const unshifted = [false];
unshifted.unshift(true);
if (unshifted[0]) log('LIVE_unshift');
const shifted = [false, true];
shifted.shift();
if (shifted[0]) log('LIVE_shift');
const reversed = [false, true];
reversed.reverse();
if (reversed[0]) log('LIVE_reverse');
const truncated = [1, 2];
truncated.length = 0;
if (truncated[0] === undefined) log('LIVE_length_write');
const grown = [1, 2];
grown.push(3);
if (grown.length === 3) log('LIVE_length_read');
const original = [false];
const alias = original;
alias[0] = true;
if (original[0]) log('LIVE_alias_write');

// A write through a callback parameter, a spread argument, a `for...of` binding, a
// destructured element, and a found element changes the element.
const viaCallback = [{ flag: false }];
viaCallback.forEach((item) => {
  item.flag = true;
});
if (viaCallback[0].flag) log('LIVE_callback_write');
const viaSpread = [{ flag: false }];
function setFlag(item) {
  item.flag = true;
}
setFlag(...viaSpread);
if (viaSpread[0].flag) log('LIVE_spread_write');
const viaLoop = [{ flag: false }];
for (const item of viaLoop) item.flag = true;
if (viaLoop[0].flag) log('LIVE_for_of_write');
const viaPattern = [{ flag: false }];
const [item] = viaPattern;
item.flag = true;
if (viaPattern[0].flag) log('LIVE_destructure_write');
const viaFind = [{ flag: false }];
viaFind.find(() => true).flag = true;
if (viaFind[0].flag) log('LIVE_find_write');

// A callback that the analysis also sees in a direct call, and the value of a method call.
function callback(x) {
  if (x) log('LIVE_recorded_callback');
}
callback(false);
[true].forEach(callback);
const mapped = [false].map(() => true);
if (mapped[0]) log('LIVE_map_result');

// Conversions, and a `constructor` that a write sets, run code.
const joined = ['x'];
joined[0] = {
  toString() {
    log('LIVE_join_element');
    return 'y';
  },
};
joined.join(',');
[1, 2].join({
  toString() {
    log('LIVE_join_separator');
    return ',';
  },
});
[1].at({
  valueOf() {
    log('LIVE_at_argument');
    return 0;
  },
});
const species = [1];
species.constructor = {
  [Symbol.species]: function () {
    log('LIVE_species');
    return {};
  },
};
species.slice();

// Code can add a key to `Array.prototype`.
Array.prototype.addedKey = true;
if ([].addedKey) log('LIVE_array_prototype_key');
delete Array.prototype.addedKey;

export const done = true;
