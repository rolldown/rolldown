# Reason
1. Since the `sideEffects: false`, and the `ImportDeclaration` is just plain, the whole sub tree (including css file) should be eliminated
# Diff
## /out/test.js
### esbuild
```js
// project/node_modules/pkg/button.jsx
var Button = () => /* @__PURE__ */ React.createElement("button", null);

// project/test.jsx
render(/* @__PURE__ */ React.createElement(Button, null));
```
### rolldown
```js

//#region node_modules/pkg/button.jsx
const Button$1 = () => /* @__PURE__ */ React.createElement("button", null);

//#endregion
//#region test.jsx
render(/* @__PURE__ */ React.createElement(Button$1, null));

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out/test.js
+++ rolldown	test.js
@@ -1,2 +1,2 @@
-var Button = () => React.createElement("button", null);
-render(React.createElement(Button, null));
+var Button$1 = () => React.createElement("button", null);
+render(React.createElement(Button$1, null));

```
## /out/test.css
### esbuild
```js
/* project/node_modules/pkg/styles.css */
button {
  color: red;
}
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/test.css
+++ rolldown	
@@ -1,4 +0,0 @@
-/* project/node_modules/pkg/styles.css */
-button {
-  color: red;
-}
\ No newline at end of file

```