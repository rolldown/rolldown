# Diff
## /out.js
### esbuild
```js
class Foo {
  #foo = 123;
  #bar;
  foo = 123;
  bar;
  static #s_foo = 123;
  static #s_bar;
  static s_foo = 123;
  static s_bar;
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
@@ -1,10 +0,0 @@
-class Foo {
-    #foo = 123;
-    #bar;
-    foo = 123;
-    bar;
-    static #s_foo = 123;
-    static #s_bar;
-    static s_foo = 123;
-    static s_bar;
-}

```