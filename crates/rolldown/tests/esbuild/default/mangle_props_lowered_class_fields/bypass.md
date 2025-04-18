# Reason
1. could be done in minifier
# Diff
## /out.js
### esbuild
```js
class Foo {
  constructor() {
    __publicField(this, "a", 123);
  }
}
__publicField(Foo, "b", 234);
Foo.b = new Foo().a;
```
### rolldown
```js
//#region entry.js
var Foo = class {
	foo_ = 123;
	static bar_ = 234;
};
Foo.bar_ = new Foo().foo_;

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,7 +1,5 @@
-class Foo {
-    constructor() {
-        __publicField(this, "a", 123);
-    }
-}
-__publicField(Foo, "b", 234);
-Foo.b = new Foo().a;
+var Foo = class {
+    foo_ = 123;
+    static bar_ = 234;
+};
+Foo.bar_ = new Foo().foo_;

```