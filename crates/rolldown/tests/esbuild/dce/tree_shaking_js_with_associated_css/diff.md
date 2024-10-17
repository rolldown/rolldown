# Reason
1. needs css stable
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
import { jsx as _jsx } from "react/jsx-runtime";

//#region node_modules/pkg/button.js
let Button;

//#endregion
//#region test.jsx
render(_jsx(Button, {}));

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out/test.js
+++ rolldown	test.js
@@ -1,2 +1,3 @@
+import {jsx as _jsx} from "react/jsx-runtime";
 var Button;
-render(React.createElement(Button, null));
+render(_jsx(Button, {}));

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
button { color: red }

```
### diff
```diff
===================================================================
--- esbuild	/out/test.css
+++ rolldown	test.css
@@ -1,9 +1,1 @@
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
+button { color: red }

```