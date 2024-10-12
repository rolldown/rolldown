# Diff
## /out.js
### esbuild
```js
class Foo {
}
class Bar extends Foo {
  constructor() {
    super();
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
@@ -1,6 +0,0 @@
-class Foo {}
-class Bar extends Foo {
-    constructor() {
-        super();
-    }
-}

```