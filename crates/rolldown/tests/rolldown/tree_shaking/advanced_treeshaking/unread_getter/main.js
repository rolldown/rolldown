const o = {
  get a() {
    console.log('DEAD_GETTER');
    return 1;
  },
  b: 2,
};
o.b;
export const done = true;
