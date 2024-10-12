# Diff
## /out.js
### esbuild
```js
class Foo {
  #foo;
  #bar;
  baz() {
    return [
      this.#foo,
      this.#bar,
      #foo in this
    ];
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
@@ -1,7 +0,0 @@
-class Foo {
-    #foo;
-    #bar;
-    baz() {
-        return [this.#foo, this.#bar, (#foo in this)];
-    }
-}

```