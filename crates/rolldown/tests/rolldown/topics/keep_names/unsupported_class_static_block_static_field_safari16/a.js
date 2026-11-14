export class Foo {
  static tag = this.name;
  static tag2 = Foo.name;
}
export const Bar = class {
  static tag = this.name;
};
export default class {
  static tag = this.name;
}
