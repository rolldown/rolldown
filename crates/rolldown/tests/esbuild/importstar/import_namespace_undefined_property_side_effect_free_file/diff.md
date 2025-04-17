# Reason
1. Wrong output
# Diff
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
import { import_no_side_effects } from "./no-side-effects.js";

//#region entry-nope.js
console.log(void 0, void 0, import_no_side_effects.nope);

```
### diff
```diff
===================================================================
--- esbuild	/out/entry-nope.js
+++ rolldown	entry-nope.js
@@ -1,13 +1,2 @@
-var require_no_side_effects = __commonJS({
-    "foo/no-side-effects.js"() {
-        console.log("js");
-    }
-});
-var require_no_side_effects2 = __commonJS({
-    "foo/no-side-effects.cjs"() {
-        console.log("cjs");
-    }
-});
-var js = __toESM(require_no_side_effects());
-var cjs = __toESM(require_no_side_effects2());
-console.log(void 0, void 0, void 0);
+import {import_no_side_effects} from "./no-side-effects.js";
+console.log(void 0, void 0, import_no_side_effects.nope);

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
import { import_no_side_effects } from "./no-side-effects.js";

//#region entry-default.js
console.log(void 0, void 0, import_no_side_effects.default);

```
### diff
```diff
===================================================================
--- esbuild	/out/entry-default.js
+++ rolldown	entry-default.js
@@ -1,13 +1,2 @@
-var require_no_side_effects = __commonJS({
-    "foo/no-side-effects.js"() {
-        console.log("js");
-    }
-});
-var require_no_side_effects2 = __commonJS({
-    "foo/no-side-effects.cjs"() {
-        console.log("cjs");
-    }
-});
-var js = __toESM(require_no_side_effects());
-var cjs = __toESM(require_no_side_effects2());
-console.log(js.default, void 0, cjs.default);
+import {import_no_side_effects} from "./no-side-effects.js";
+console.log(void 0, void 0, import_no_side_effects.default);

```