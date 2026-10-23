import { Shared } from './shared.js';

const Expression = function () {};
const holder = { Ctor: function () {} };
class Base {}
class Derived extends Base {}
class WithField extends Base {
  field = 1;
}
class GrandChild extends Derived {}

export function main() {
  new Expression('DEAD_function_expression');
  new holder.Ctor('DEAD_holder');
  new Derived('DEAD_derived');
  new WithField('DEAD_derived_field');
  new GrandChild('DEAD_grand_child');
  new Shared('DEAD_shared_chunk');
  return 1;
}
