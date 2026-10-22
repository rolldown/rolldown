const counter = {
  n: 0,
  untouched: 1,
  inc() {
    this.n++;
    return this.n + this.untouched;
  },
};
export const once = counter.inc();
