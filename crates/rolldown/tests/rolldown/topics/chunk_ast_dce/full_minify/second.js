export class Second {
  #unused = 'padding';
  #value = 'second';
  #suffix = '-outer';

  getInner() {
    const outer = this;
    return class Inner {
      #prefix = 'inner';
      #unused = 'different-layout';

      get() {
        return this.#prefix + outer.#suffix;
      }
    };
  }
}
