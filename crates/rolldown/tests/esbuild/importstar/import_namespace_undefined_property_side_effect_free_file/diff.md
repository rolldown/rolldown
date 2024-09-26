## /out/entry-nope.js
### esbuild
```js
// foo/no-side-effects.js
var require_no_side_effects = __commonJS({
  "foo/no-side-effects.js"() {
    console.log("js");
  }
});

// foo/no-side-effects.cjs
var require_no_side_effects2 = __commonJS({
  "foo/no-side-effects.cjs"() {
    console.log("cjs");
  }
});

// entry-nope.js
var js = __toESM(require_no_side_effects());
var cjs = __toESM(require_no_side_effects2());
console.log(
  void 0,
  void 0,
  void 0
);
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/entry-nope.js
+++ rolldown	
@@ -1,13 +0,0 @@
-var require_no_side_effects = __commonJS({
-    'foo/no-side-effects.js'() {
-        console.log('js');
-    }
-});
-var require_no_side_effects2 = __commonJS({
-    'foo/no-side-effects.cjs'() {
-        console.log('cjs');
-    }
-});
-var js = __toESM(require_no_side_effects());
-var cjs = __toESM(require_no_side_effects2());
-console.log(void 0, void 0, void 0);
\ No newline at end of file

```
## /out/entry-default.js
### esbuild
```js
// foo/no-side-effects.js
var require_no_side_effects = __commonJS({
  "foo/no-side-effects.js"() {
    console.log("js");
  }
});

// foo/no-side-effects.cjs
var require_no_side_effects2 = __commonJS({
  "foo/no-side-effects.cjs"() {
    console.log("cjs");
  }
});

// entry-default.js
var js = __toESM(require_no_side_effects());
var cjs = __toESM(require_no_side_effects2());
console.log(
  js.default,
  void 0,
  cjs.default
);
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/entry-default.js
+++ rolldown	
@@ -1,13 +0,0 @@
-var require_no_side_effects = __commonJS({
-    'foo/no-side-effects.js'() {
-        console.log('js');
-    }
-});
-var require_no_side_effects2 = __commonJS({
-    'foo/no-side-effects.cjs'() {
-        console.log('cjs');
-    }
-});
-var js = __toESM(require_no_side_effects());
-var cjs = __toESM(require_no_side_effects2());
-console.log(js.default, void 0, cjs.default);
\ No newline at end of file

```
