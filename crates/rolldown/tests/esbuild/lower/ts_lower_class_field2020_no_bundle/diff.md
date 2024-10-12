# Diff
## /out.js
### esbuild
```js
var _foo, _bar, _s_foo, _s_bar;
class Foo {
  constructor() {
    __privateAdd(this, _foo, 123);
    __privateAdd(this, _bar);
    this.foo = 123;
  }
}
_foo = new WeakMap();
_bar = new WeakMap();
_s_foo = new WeakMap();
_s_bar = new WeakMap();
__privateAdd(Foo, _s_foo, 123);
__privateAdd(Foo, _s_bar);
Foo.s_foo = 123;
```
### rolldown
```js


```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,15 +0,0 @@
-var _foo, _bar, _s_foo, _s_bar;
-class Foo {
-    constructor() {
-        __privateAdd(this, _foo, 123);
-        __privateAdd(this, _bar);
-        this.foo = 123;
-    }
-}
-_foo = new WeakMap();
-_bar = new WeakMap();
-_s_foo = new WeakMap();
-_s_bar = new WeakMap();
-__privateAdd(Foo, _s_foo, 123);
-__privateAdd(Foo, _s_bar);
-Foo.s_foo = 123;

```