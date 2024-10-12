# Diff
## /out.js
### esbuild
```js
// entry.ts
var Foo = class {
  100 = 100;
  200 = 200;
  300 = 300;
  "str 1" = "str 1" /* STR */;
  123 = 123 /* NUM */;
  "str 2" = "str 2" /* STR */;
  321 = 321 /* NUM */;
};
shouldNotBeComputed(
  class {
    100 = 100;
    200 = 200;
    300 = 300;
    "str 1" = "str 1" /* STR */;
    123 = 123 /* NUM */;
    "str 2" = "str 2" /* STR */;
    321 = 321 /* NUM */;
  },
  {
    100: 100,
    200: 200,
    300: 300,
    "str 1": "str 1" /* STR */,
    123: 123 /* NUM */,
    "str 2": "str 2" /* STR */,
    321: 321 /* NUM */
  }
);
mustBeComputed(
  { ["__proto__"]: null },
  { ["__proto__"]: null },
  class {
    ["constructor"]() {
    }
  },
  class {
    ["constructor"]() {
    }
  },
  class {
    static ["prototype"]() {
    }
  },
  class {
    static ["prototype"]() {
    }
  }
);
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	
@@ -1,39 +0,0 @@
-var Foo = class {
-    100 = 100;
-    200 = 200;
-    300 = 300;
-    "str 1" = "str 1";
-    123 = 123;
-    "str 2" = "str 2";
-    321 = 321;
-};
-shouldNotBeComputed(class {
-    100 = 100;
-    200 = 200;
-    300 = 300;
-    "str 1" = "str 1";
-    123 = 123;
-    "str 2" = "str 2";
-    321 = 321;
-}, {
-    100: 100,
-    200: 200,
-    300: 300,
-    "str 1": "str 1",
-    123: 123,
-    "str 2": "str 2",
-    321: 321
-});
-mustBeComputed({
-    ["__proto__"]: null
-}, {
-    ["__proto__"]: null
-}, class {
-    ["constructor"]() {}
-}, class {
-    ["constructor"]() {}
-}, class {
-    static ["prototype"]() {}
-}, class {
-    static ["prototype"]() {}
-});

```