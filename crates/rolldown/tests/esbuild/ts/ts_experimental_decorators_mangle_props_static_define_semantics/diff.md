# Reason
1. lowering ts experimental decorator
# Diff
## /out.js
### esbuild
```js
// entry.ts
var Foo = class {
  static prop1 = null;
  static a = null;
  static ["prop3"] = null;
  static ["prop4_"] = null;
  static [/* @__KEY__ */ "prop5"] = null;
  static [/* @__KEY__ */ "b"] = null;
};
__decorateClass([
  dec(1)
], Foo, "prop1", 2);
__decorateClass([
  dec(2)
], Foo, "a", 2);
__decorateClass([
  dec(3)
], Foo, "prop3", 2);
__decorateClass([
  dec(4)
], Foo, "prop4_", 2);
__decorateClass([
  dec(5)
], Foo, /* @__KEY__ */ "prop5", 2);
__decorateClass([
  dec(6)
], Foo, /* @__KEY__ */ "b", 2);
```
### rolldown
```js
//#region entry.ts
var Foo = class {
	@dec(1) static prop1 = null;
	@dec(2) static prop2_ = null;
	@dec(3) static ["prop3"] = null;
	@dec(4) static ["prop4_"] = null;
	@dec(5) static ["prop5"] = null;
	@dec(6) static ["prop6_"] = null;
};

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,14 +1,11 @@
+//#region entry.ts
 var Foo = class {
-    static prop1 = null;
-    static a = null;
-    static ["prop3"] = null;
-    static ["prop4_"] = null;
-    static ["prop5"] = null;
-    static ["b"] = null;
+	@dec(1) static prop1 = null;
+	@dec(2) static prop2_ = null;
+	@dec(3) static ["prop3"] = null;
+	@dec(4) static ["prop4_"] = null;
+	@dec(5) static ["prop5"] = null;
+	@dec(6) static ["prop6_"] = null;
 };
-__decorateClass([dec(1)], Foo, "prop1", 2);
-__decorateClass([dec(2)], Foo, "a", 2);
-__decorateClass([dec(3)], Foo, "prop3", 2);
-__decorateClass([dec(4)], Foo, "prop4_", 2);
-__decorateClass([dec(5)], Foo, "prop5", 2);
-__decorateClass([dec(6)], Foo, "b", 2);
+
+//#endregion
\ No newline at end of file

```