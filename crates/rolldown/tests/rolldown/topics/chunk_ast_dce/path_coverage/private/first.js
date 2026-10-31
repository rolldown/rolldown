globalThis.__pathEvents.push('private-first-anchor');
export class First {
  #value = 'first-value';
  read() {
    return this.#value;
  }
}
