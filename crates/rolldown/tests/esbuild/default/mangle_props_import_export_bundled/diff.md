# Diff
## /out/entry-esm.js
### esbuild
```js
// cjs.js
var require_cjs = __commonJS({
  "cjs.js"(exports) {
    exports.a = "foo";
  }
});

// esm.js
var esm_exports = {};
__export(esm_exports, {
  esm_foo_: () => esm_foo_
});
var esm_foo_ = "foo";

// entry-esm.js
var import_cjs = __toESM(require_cjs());
var cjs = __toESM(require_cjs());
var bar_ = [
  esm_foo_,
  import_cjs.cjs_foo_,
  esm_exports.b,
  cjs.a
];
export {
  bar_
};
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/entry-esm.js
+++ rolldown	
@@ -1,14 +0,0 @@
-var require_cjs = __commonJS({
-    "cjs.js"(exports) {
-        exports.a = "foo";
-    }
-});
-var esm_exports = {};
-__export(esm_exports, {
-    esm_foo_: () => esm_foo_
-});
-var esm_foo_ = "foo";
-var import_cjs = __toESM(require_cjs());
-var cjs = __toESM(require_cjs());
-var bar_ = [esm_foo_, import_cjs.cjs_foo_, esm_exports.b, cjs.a];
-export {bar_};

```
## /out/entry-cjs.js
### esbuild
```js
// esm.js
var esm_exports = {};
__export(esm_exports, {
  esm_foo_: () => esm_foo_
});
var esm_foo_;
var init_esm = __esm({
  "esm.js"() {
    esm_foo_ = "foo";
  }
});

// cjs.js
var require_cjs = __commonJS({
  "cjs.js"(exports) {
    exports.a = "foo";
  }
});

// entry-cjs.js
var require_entry_cjs = __commonJS({
  "entry-cjs.js"(exports) {
    var { b: esm_foo_2 } = (init_esm(), __toCommonJS(esm_exports));
    var { a: cjs_foo_ } = require_cjs();
    exports.c = [
      esm_foo_2,
      cjs_foo_
    ];
  }
});
export default require_entry_cjs();
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/entry-cjs.js
+++ rolldown	
@@ -1,23 +0,0 @@
-var esm_exports = {};
-__export(esm_exports, {
-    esm_foo_: () => esm_foo_
-});
-var esm_foo_;
-var init_esm = __esm({
-    "esm.js"() {
-        esm_foo_ = "foo";
-    }
-});
-var require_cjs = __commonJS({
-    "cjs.js"(exports) {
-        exports.a = "foo";
-    }
-});
-var require_entry_cjs = __commonJS({
-    "entry-cjs.js"(exports) {
-        var {b: esm_foo_2} = (init_esm(), __toCommonJS(esm_exports));
-        var {a: cjs_foo_} = require_cjs();
-        exports.c = [esm_foo_2, cjs_foo_];
-    }
-});
-export default require_entry_cjs();

```