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


```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,8 +0,0 @@
-class Foo extends Bar {
-    constructor() {
-        super();
-        this.foo = 1;
-        this.bar = 2;
-        (foo(), bar());
-    }
-}

```