# Reason
1. lowering ts experimental decorator
# Diff
## /out.js
### esbuild
```js
// entry.ts
var Foo = class {
  prop1 = null;
  a = null;
  ["prop3"] = null;
  ["prop4_"] = null;
  [/* @__KEY__ */ "prop5"] = null;
  [/* @__KEY__ */ "b"] = null;
};
__decorateClass([
  dec(1)
], Foo.prototype, "prop1", 2);
__decorateClass([
  dec(2)
], Foo.prototype, "a", 2);
__decorateClass([
  dec(3)
], Foo.prototype, "prop3", 2);
__decorateClass([
  dec(4)
], Foo.prototype, "prop4_", 2);
__decorateClass([
  dec(5)
], Foo.prototype, /* @__KEY__ */ "prop5", 2);
__decorateClass([
  dec(6)
], Foo.prototype, /* @__KEY__ */ "b", 2);
```
### rolldown
```js

//#region entry.ts
var Foo = class {
	@dec(1) prop1 = null;
	@dec(2) prop2_ = null;
	@dec(3) ["prop3"] = null;
	@dec(4) ["prop4_"] = null;
	@dec(5) ["prop5"] = null;
	@dec(6) ["prop6_"] = null;
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
-    prop1 = null;
-    a = null;
-    ["prop3"] = null;
-    ["prop4_"] = null;
-    ["prop5"] = null;
-    ["b"] = null;
+	@dec(1) prop1 = null;
+	@dec(2) prop2_ = null;
+	@dec(3) ["prop3"] = null;
+	@dec(4) ["prop4_"] = null;
+	@dec(5) ["prop5"] = null;
+	@dec(6) ["prop6_"] = null;
 };
-__decorateClass([dec(1)], Foo.prototype, "prop1", 2);
-__decorateClass([dec(2)], Foo.prototype, "a", 2);
-__decorateClass([dec(3)], Foo.prototype, "prop3", 2);
-__decorateClass([dec(4)], Foo.prototype, "prop4_", 2);
-__decorateClass([dec(5)], Foo.prototype, "prop5", 2);
-__decorateClass([dec(6)], Foo.prototype, "b", 2);
+//#endregion

```