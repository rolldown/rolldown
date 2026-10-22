globalThis.log = [];
const o = {
  get a() {
    globalThis.log.push('getter');
    return 1;
  },
  DEAD_B: 2,
};
o.a;
export const keep = 1;
