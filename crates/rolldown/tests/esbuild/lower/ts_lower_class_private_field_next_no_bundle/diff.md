# Diff
## /out.js
### esbuild
```js
class Foo {
  constructor() {
    this.#foo = 123;
    this.foo = 123;
  }
  #foo;
  #bar;
  static #s_foo = 123;
  static #s_bar;
  static {
    this.s_foo = 123;
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
@@ -1,13 +0,0 @@
-class Foo {
-    constructor() {
-        this.#foo = 123;
-        this.foo = 123;
-    }
-    #foo;
-    #bar;
-    static #s_foo = 123;
-    static #s_bar;
-    static {
-        this.s_foo = 123;
-    }
-}

```