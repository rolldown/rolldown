export class First {
  #value = 'first';

  get() {
    return this.#value;
  }
}

export const inlined = { value: 'inlined' };
