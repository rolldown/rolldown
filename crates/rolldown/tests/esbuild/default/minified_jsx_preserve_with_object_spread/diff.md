# Reason
1. not support preserve `jsx`
# Diff
## /out.js
### esbuild
```js
// entry.jsx
var obj = {
  before,
  [key]: value,
  key: value,
  after
};
<Foo
  before
  {...{ [key]: value }}
  key={value}
  after
/>;
<Bar
  a={a}
  {...{ [b]: c }}
  {...d}
  e={e}
/>;
```
### rolldown
```js
import { jsx } from "react/jsx-runtime";

//#region entry.jsx
const obj = {
	before,
	...{ [key]: value },
	...{ key: value },
	after
};
jsx(Foo, {
	before: true,
	[key]: value,
	key: value,
	after: true
});
jsx(Bar, {
	a,
	[b]: c,
	...d,
	e
});

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,19 +1,23 @@
-// entry.jsx
-var obj = {
-  before,
-  [key]: value,
-  key: value,
-  after
+import { jsx } from "react/jsx-runtime";
+
+//#region entry.jsx
+const obj = {
+	before,
+	...{ [key]: value },
+	...{ key: value },
+	after
 };
-<Foo
-  before
-  {...{ [key]: value }}
-  key={value}
-  after
-/>;
-<Bar
-  a={a}
-  {...{ [b]: c }}
-  {...d}
-  e={e}
-/>;
\ No newline at end of file
+jsx(Foo, {
+	before: true,
+	[key]: value,
+	key: value,
+	after: true
+});
+jsx(Bar, {
+	a,
+	[b]: c,
+	...d,
+	e
+});
+
+//#endregion
\ No newline at end of file

```