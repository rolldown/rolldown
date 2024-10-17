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
import { __toESM, require_no_side_effects, require_no_side_effects$1 } from "./no-side-effects.js";

//#region entry-nope.js
var import_no_side_effects = __toESM(require_no_side_effects$1());
var import_no_side_effects$1 = __toESM(require_no_side_effects());
console.log(import_no_side_effects.nope, void 0, import_no_side_effects$1.nope);

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out/entry-nope.js
+++ rolldown	entry-nope.js
@@ -1,13 +1,4 @@
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
+import {__toESM, require_no_side_effects, require_no_side_effects$1} from "./no-side-effects.js";
+var import_no_side_effects = __toESM(require_no_side_effects$1());
+var import_no_side_effects$1 = __toESM(require_no_side_effects());
+console.log(import_no_side_effects.nope, void 0, import_no_side_effects$1.nope);

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
import { __toESM, require_no_side_effects, require_no_side_effects$1 } from "./no-side-effects.js";

//#region entry-default.js
var import_no_side_effects = __toESM(require_no_side_effects$1());
var import_no_side_effects$1 = __toESM(require_no_side_effects());
console.log(import_no_side_effects.default, void 0, import_no_side_effects$1.default);

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out/entry-default.js
+++ rolldown	entry-default.js
@@ -1,13 +1,4 @@
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
+import {__toESM, require_no_side_effects, require_no_side_effects$1} from "./no-side-effects.js";
+var import_no_side_effects = __toESM(require_no_side_effects$1());
+var import_no_side_effects$1 = __toESM(require_no_side_effects());
+console.log(import_no_side_effects.default, void 0, import_no_side_effects$1.default);

```