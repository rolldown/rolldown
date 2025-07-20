# Reason
1. different fs
# Diff
## /out/test.js
### esbuild
```js
// project/node_modules/pkg/button.css
var require_button = __commonJS({
  "project/node_modules/pkg/button.css"(exports, module) {
    module.exports = {};
  }
});

// project/node_modules/pkg/components.jsx
require_button();
var Button = () => /* @__PURE__ */ React.createElement("button", null);

// project/test.jsx
render(/* @__PURE__ */ React.createElement(Button, null));
```
### rolldown
```js
// HIDDEN [rolldown:runtime]
//#region node_modules/pkg/button.css
var require_button = __commonJS({ "node_modules/pkg/button.css"(exports, module) {
	module.exports = {};
} });

//#endregion
//#region node_modules/pkg/components.jsx
require_button();
const Button = () => /* @__PURE__ */ React.createElement("button", null);

//#endregion
//#region test.jsx
render(/* @__PURE__ */ React.createElement(Button, null));

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out/test.js
+++ rolldown	test.js
@@ -1,6 +1,6 @@
 var require_button = __commonJS({
-    "project/node_modules/pkg/button.css"(exports, module) {
+    "node_modules/pkg/button.css"(exports, module) {
         module.exports = {};
     }
 });
 require_button();

```
## /out/test.css
### esbuild
```js
/* project/node_modules/pkg/button.css */
button {
  color: red;
}
```
### rolldown
```js
button { color: red }

```
### diff
```diff
===================================================================
--- esbuild	/out/test.css
+++ rolldown	test.css
@@ -1,4 +1,1 @@
-/* project/node_modules/pkg/button.css */
-button {
-  color: red;
-}
\ No newline at end of file
+button { color: red }

```