# Diff
## /out.js
### esbuild
```js
var _foo;
class Foo {
  constructor() {
    __privateAdd(this, _foo);
    this.#bar = void 0;
  }
  #bar;
  baz() {
    return [
      __privateGet(this, _foo),
      this.#bar,
      __privateIn(_foo, this)
    ];
  }
}
_foo = new WeakMap();
```
### rolldown
```js


```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,12 +0,0 @@
-var _foo;
-class Foo {
-    constructor() {
-        __privateAdd(this, _foo);
-        this.#bar = void 0;
-    }
-    #bar;
-    baz() {
-        return [__privateGet(this, _foo), this.#bar, __privateIn(_foo, this)];
-    }
-}
-_foo = new WeakMap();

```