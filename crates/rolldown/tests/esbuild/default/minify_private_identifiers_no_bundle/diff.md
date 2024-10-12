# Diff
## /out.js
### esbuild
```js
class Foo {
  #a;
  foo = class {
    #s;
    #f;
    #r;
  };
  get #o() {
  }
  set #o(a) {
  }
}
class Bar {
  #a;
  foo = class {
    #s;
    #f;
    #r;
  };
  get #o() {
  }
  set #o(a) {
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
@@ -1,20 +0,0 @@
-class Foo {
-    #a;
-    foo = class {
-        #s;
-        #f;
-        #r;
-    };
-    get #o() {}
-    set #o(a) {}
-}
-class Bar {
-    #a;
-    foo = class {
-        #s;
-        #f;
-        #r;
-    };
-    get #o() {}
-    set #o(a) {}
-}

```