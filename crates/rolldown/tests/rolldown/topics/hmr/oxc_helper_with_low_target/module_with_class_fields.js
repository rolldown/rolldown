class MyService {
  name = 'service';
  #privateField = 42;
  getValue() {
    return this.#privateField;
  }
}
export function getValue() {
  return new MyService().getValue()
}
