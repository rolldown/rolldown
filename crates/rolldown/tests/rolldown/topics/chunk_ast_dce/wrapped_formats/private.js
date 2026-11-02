import external, * as namespace from 'wrapped-external';

globalThis.__wrappedEvents.push('private-wrapper-anchor');

class First {
  #value = 'first';
  read() {
    return this.#value;
  }
}

class Second {
  #value = 'second';
  read() {
    class Nested {
      #value = 'nested';
      read() {
        return this.#value;
      }
    }
    return [this.#value, new Nested().read()];
  }
}

export function read() {
  return {
    value: external(globalThis.__wrappedInput),
    offset: namespace.offset,
    sameDefault: namespace.default === external,
    privateValues: [new First().read(), ...new Second().read()],
  };
}
