export const one = 1;
export const negativeZero = -0;
export const nan = NaN;
export const astral = '\u{10000}';
export const lastBmp = '￿';
export const ten = '10';
export const nine = '9';
export const upper = 'B';
export const lower = 'a';
export const object = {
  valueOf() {
    globalThis.log.push('LIVE_value_of');
    return 1;
  },
};
export const named = {
  toString() {
    globalThis.log.push('LIVE_to_string');
    return 'm';
  },
};
