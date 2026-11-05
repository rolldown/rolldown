export class Foo {}
export const Bar = class {};
export const Baz = class Baz {};
export default class {}
export class SelfRef {
  static self() {
    return SelfRef;
  }
}
export const Named =
  (0,
  class Named {
    static self() {
      return Named;
    }
  });
