# Reason
1. lower decorator
# Diff
## /out/keep-these.js
### esbuild
```js
// decorator.ts
var fn = () => {
  console.log("side effect");
};

// keep-these.ts
var Class = class {
};
Class = __decorateClass([
  fn
], Class);
var Field = class {
  field;
};
__decorateClass([
  fn
], Field.prototype, "field", 2);
var Method = class {
  method() {
  }
};
__decorateClass([
  fn
], Method.prototype, "method", 1);
var Accessor = class {
  accessor accessor;
};
__decorateClass([
  fn
], Accessor.prototype, "accessor", 1);
var Parameter = class {
  foo(bar) {
  }
};
__decorateClass([
  __decorateParam(0, fn)
], Parameter.prototype, "foo", 1);
var StaticField = class {
  static field;
};
__decorateClass([
  fn
], StaticField, "field", 2);
var StaticMethod = class {
  static method() {
  }
};
__decorateClass([
  fn
], StaticMethod, "method", 1);
var StaticAccessor = class {
  static accessor accessor;
};
__decorateClass([
  fn
], StaticAccessor, "accessor", 1);
var StaticParameter = class {
  static foo(bar) {
  }
};
__decorateClass([
  __decorateParam(0, fn)
], StaticParameter, "foo", 1);
```
### rolldown
```js

//#region decorator.ts
const fn = () => {
	console.log("side effect");
};

//#endregion
//#region keep-these.ts
@fn class Class {}
class Field {
	@fn field;
}
class Method {
	@fn method() {}
}
class Parameter {
	foo(@fn bar) {}
}
class StaticField {
	@fn static field;
}
class StaticMethod {
	@fn static method() {}
}
class StaticParameter {
	static foo(@fn bar) {}
}

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out/keep-these.js
+++ rolldown	keep-these.js
@@ -1,63 +1,29 @@
-// decorator.ts
-var fn = () => {
-  console.log("side effect");
-};
 
-// keep-these.ts
-var Class = class {
+//#region decorator.ts
+const fn = () => {
+	console.log("side effect");
 };
-Class = __decorateClass([
-  fn
-], Class);
-var Field = class {
-  field;
-};
-__decorateClass([
-  fn
-], Field.prototype, "field", 2);
-var Method = class {
-  method() {
-  }
-};
-__decorateClass([
-  fn
-], Method.prototype, "method", 1);
-var Accessor = class {
-  accessor accessor;
-};
-__decorateClass([
-  fn
-], Accessor.prototype, "accessor", 1);
-var Parameter = class {
-  foo(bar) {
-  }
-};
-__decorateClass([
-  __decorateParam(0, fn)
-], Parameter.prototype, "foo", 1);
-var StaticField = class {
-  static field;
-};
-__decorateClass([
-  fn
-], StaticField, "field", 2);
-var StaticMethod = class {
-  static method() {
-  }
-};
-__decorateClass([
-  fn
-], StaticMethod, "method", 1);
-var StaticAccessor = class {
-  static accessor accessor;
-};
-__decorateClass([
-  fn
-], StaticAccessor, "accessor", 1);
-var StaticParameter = class {
-  static foo(bar) {
-  }
-};
-__decorateClass([
-  __decorateParam(0, fn)
-], StaticParameter, "foo", 1);
\ No newline at end of file
+
+//#endregion
+//#region keep-these.ts
+@fn class Class {}
+class Field {
+	@fn field;
+}
+class Method {
+	@fn method() {}
+}
+class Parameter {
+	foo(@fn bar) {}
+}
+class StaticField {
+	@fn static field;
+}
+class StaticMethod {
+	@fn static method() {}
+}
+class StaticParameter {
+	static foo(@fn bar) {}
+}
+
+//#endregion
\ No newline at end of file

```