globalThis.__pathEvents.push('private-second-anchor');
export class Second {
  #padding = 'different-layout';
  #value = 'second-value';
  #suffix = 'second-suffix';
  read() {
    return this.#value;
  }
  inner() {
    const outer = this;
    return class Inner {
      #value = 'inner-value';
      #padding = 'nested-layout';
      read() {
        return this.#value + ':' + outer.#suffix;
      }
    };
  }
}
