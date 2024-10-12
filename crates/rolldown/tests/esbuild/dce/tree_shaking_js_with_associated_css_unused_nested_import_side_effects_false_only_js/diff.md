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

```
### diff
```diff
===================================================================
--- esbuild	/out/test.js
+++ rolldown	
@@ -1,2 +0,0 @@
-var Button = () => React.createElement("button", null);
-render(React.createElement(Button, null));

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