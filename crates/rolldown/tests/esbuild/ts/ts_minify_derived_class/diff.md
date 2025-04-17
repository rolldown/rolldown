# Reason
1. lowering class
# Diff
## /out.js
### esbuild
```js
class Foo extends Bar {
  constructor() {
    super();
    this.foo = 1;
    this.bar = 2;
    foo(), bar();
  }
}
```
### rolldown
```js

//#region entry.ts
var Foo = class extends Bar {
	foo = 1;
	bar = 2;
	constructor() {
		super();
		foo();
		bar();
	}
};

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,8 +1,9 @@
-class Foo extends Bar {
+var Foo = class extends Bar {
+    foo = 1;
+    bar = 2;
     constructor() {
         super();
-        this.foo = 1;
-        this.bar = 2;
-        (foo(), bar());
+        foo();
+        bar();
     }
-}
+};

```