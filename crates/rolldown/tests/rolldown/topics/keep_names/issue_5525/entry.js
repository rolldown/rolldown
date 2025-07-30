import {
	Foo as Bar,
	VarKlass as VarKlass2,
	FunctionDeclaration as FunctionDeclaration2,
	VarFunction as VarFunction2,
	VarKlass,
} from "./foo.js";
import assert from "node:assert";

assert.strictEqual(Bar.name, "Foo");
assert.strictEqual(VarKlass2.name, "VarKlass");
assert.strictEqual(FunctionDeclaration2.name, "FunctionDeclaration");
assert.strictEqual(VarFunction2.name, "VarFunction");

class Foo {}
export const VarKlass = class {};
export function FunctionDeclaration() {}
export const VarFunction = function () {};

assert.strictEqual(Foo.name, "Foo");
assert.strictEqual(VarKlass.name, "VarKlass");
assert.strictEqual(FunctionDeclaration.name, "FunctionDeclaration");
assert.strictEqual(VarFunction.name, "VarFunction");
