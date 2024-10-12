# Diff
## /out.js
### esbuild
```js
class Foo {
  #x;
  foo() {
    this?.#x.y;
    this?.y.#x;
    this.#x?.y;
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
-class Foo {
-    #x;
-    foo() {
-        this?.#x.y;
-        this?.y.#x;
-        this.#x?.y;
-    }
-}

```