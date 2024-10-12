# Diff
## /out/test.js
### esbuild
```js
// project/node_modules/pkg/button.js
var Button;

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
-var Button;
-render(React.createElement(Button, null));

```
## /out/test.css
### esbuild
```js
/* project/node_modules/pkg/button.css */
button {
  color: red;
}

/* project/node_modules/pkg/menu.css */
menu {
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
@@ -1,9 +0,0 @@
-/* project/node_modules/pkg/button.css */
-button {
-  color: red;
-}
-
-/* project/node_modules/pkg/menu.css */
-menu {
-  color: red;
-}
\ No newline at end of file

```