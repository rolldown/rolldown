class MyService {
  name = 'service-updated';
  #privateField = 99;
  getValue() {
    return this.#privateField;
  }
}
export function getValue() {
  return new MyService().getValue()
}
