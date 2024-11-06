# Reason
1. css module should be wrapped with `__commonJS`
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


//#region node_modules/pkg/button.css
var button_exports = {};
var init_button = __esm({ "node_modules/pkg/button.css"() {} });

//#endregion
//#region node_modules/pkg/components.jsx
init_button();
const Button = () => React.createElement("button", null);

//#endregion
//#region test.jsx
render(React.createElement(Button, null));

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out/test.js
+++ rolldown	test.js
@@ -1,8 +1,7 @@
-var require_button = __commonJS({
-    "project/node_modules/pkg/button.css"(exports, module) {
-        module.exports = {};
-    }
+var button_exports = {};
+var init_button = __esm({
+    "node_modules/pkg/button.css"() {}
 });
-require_button();
+init_button();
 var Button = () => React.createElement("button", null);
 render(React.createElement(Button, null));

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