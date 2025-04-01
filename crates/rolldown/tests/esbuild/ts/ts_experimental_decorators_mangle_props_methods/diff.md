# Reason
1. lowering ts experimental decorator
# Diff
## /out.js
### esbuild
```js
// entry.ts
var Foo = class {
  prop1() {
  }
  a() {
  }
  ["prop3"]() {
  }
  ["prop4_"]() {
  }
  [/* @__KEY__ */ "prop5"]() {
  }
  [/* @__KEY__ */ "b"]() {
  }
};
__decorateClass([
  dec(1)
], Foo.prototype, "prop1", 1);
__decorateClass([
  dec(2)
], Foo.prototype, "a", 1);
__decorateClass([
  dec(3)
], Foo.prototype, "prop3", 1);
__decorateClass([
  dec(4)
], Foo.prototype, "prop4_", 1);
__decorateClass([
  dec(5)
], Foo.prototype, /* @__KEY__ */ "prop5", 1);
__decorateClass([
  dec(6)
], Foo.prototype, /* @__KEY__ */ "b", 1);
```
### rolldown
```js

//#region entry.ts
var Foo = class {
	@dec(1) prop1() {}
	@dec(2) prop2_() {}
	@dec(3) ["prop3"]() {}
	@dec(4) ["prop4_"]() {}
	@dec(5) ["prop5"]() {}
	@dec(6) ["prop6_"]() {}
};
//#endregion

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,14 +1,11 @@
+
+//#region entry.ts
 var Foo = class {
-    prop1() {}
-    a() {}
-    ["prop3"]() {}
-    ["prop4_"]() {}
-    ["prop5"]() {}
-    ["b"]() {}
+	@dec(1) prop1() {}
+	@dec(2) prop2_() {}
+	@dec(3) ["prop3"]() {}
+	@dec(4) ["prop4_"]() {}
+	@dec(5) ["prop5"]() {}
+	@dec(6) ["prop6_"]() {}
 };
-__decorateClass([dec(1)], Foo.prototype, "prop1", 1);
-__decorateClass([dec(2)], Foo.prototype, "a", 1);
-__decorateClass([dec(3)], Foo.prototype, "prop3", 1);
-__decorateClass([dec(4)], Foo.prototype, "prop4_", 1);
-__decorateClass([dec(5)], Foo.prototype, "prop5", 1);
-__decorateClass([dec(6)], Foo.prototype, "b", 1);
+//#endregion

```