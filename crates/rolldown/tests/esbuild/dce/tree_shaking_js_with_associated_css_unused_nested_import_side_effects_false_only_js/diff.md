# Reason
1. needs css stable
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
import { jsx as _jsx, jsx as _jsx$1 } from "react/jsx-runtime";

//#region node_modules/pkg/button.jsx
const Button = () => _jsx$1("button", {});

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
-var Button = () => React.createElement("button", null);
-render(React.createElement(Button, null));
+import {jsx as _jsx, jsx as _jsx$1} from "react/jsx-runtime";
+var Button = () => _jsx$1("button", {});
+render(_jsx(Button, {}));

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