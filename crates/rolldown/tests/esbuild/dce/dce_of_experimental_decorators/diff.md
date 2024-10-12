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

```
### diff
```diff
===================================================================
--- esbuild	/out/keep-these.js
+++ rolldown	
@@ -1,63 +0,0 @@
-// decorator.ts
-var fn = () => {
-  console.log("side effect");
-};
-
-// keep-these.ts
-var Class = class {
-};
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

```