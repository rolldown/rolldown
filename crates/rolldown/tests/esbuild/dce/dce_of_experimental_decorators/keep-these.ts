import { fn } from './decorator'
@fn class Class {}
class Field { @fn field }
class Method { @fn method() {} }
class Accessor { @fn accessor accessor }
class Parameter { foo(@fn bar) {} }
class StaticField { @fn static field }
class StaticMethod { @fn static method() {} }
class StaticAccessor { @fn static accessor accessor }
class StaticParameter { static foo(@fn bar) {} }