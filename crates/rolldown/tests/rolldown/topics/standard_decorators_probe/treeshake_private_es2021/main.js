class Foo {
  #two = 2;
  [three()] = 3;
  static four = 4;
  static #five = 5;
  get two() {
    return this.#two;
  }
  static get five() {
    return Foo.#five;
  }
}
export const x = 1;
