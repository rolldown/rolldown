# Diff
## /out.js
### esbuild
```js
// custom-react.js
var require_custom_react = __commonJS({
  "custom-react.js"(exports, module) {
    module.exports = {};
  }
});

// entry.jsx
var import_custom_react = __toESM(require_custom_react());
console.log(/* @__PURE__ */ (0, import_custom_react.elem)("div", null), /* @__PURE__ */ (0, import_custom_react.elem)(import_custom_react.frag, null, "fragment"));
```
### rolldown
```js


//#region custom-react.js
var import_custom_react;
var require_custom_react = __commonJS({ "custom-react.js"(exports, module) {
	module.exports = {};
	import_custom_react = __toESM(require_custom_react());
} });

//#endregion
//#region entry.jsx
require_custom_react();
console.log((0, import_custom_react.elem)("div", null), (0, import_custom_react.elem)(import_custom_react.frag, null, "fragment"));

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,7 +1,9 @@
+var import_custom_react;
 var require_custom_react = __commonJS({
     "custom-react.js"(exports, module) {
         module.exports = {};
+        import_custom_react = __toESM(require_custom_react());
     }
 });
-var import_custom_react = __toESM(require_custom_react());
+require_custom_react();
 console.log((0, import_custom_react.elem)("div", null), (0, import_custom_react.elem)(import_custom_react.frag, null, "fragment"));

```