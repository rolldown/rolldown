## /out/entry.js
### esbuild
```js
// entry.jsx
console.log(
  <div x={
    /*before*/
    x
  } />,
  <div x={
    /*before*/
    "y"
  } />,
  <div x={
    /*before*/
    true
  } />,
  <div {
    /*before*/
    ...x
  } />,
  <div>{
    /*before*/
    x
  }</div>,
  <>{
    /*before*/
    x
  }</>,
  // Comments on absent AST nodes
  <div>before{}after</div>,
  <div>before{
    /* comment 1 */
    /* comment 2 */
  }after</div>,
  <div>before{
    // comment 1
    // comment 2
  }after</div>,
  <>before{}after</>,
  <>before{
    /* comment 1 */
    /* comment 2 */
  }after</>,
  <>before{
    // comment 1
    // comment 2
  }after</>
);
```
### rolldown
```js
//#region entry.jsx
console.log(<div x={x} />, <div x={"y"} />, <div x={true} />, <div {...x} />, <div>{x}</div>, <>{x}</>, <div>before{}after</div>, <div>before{}after</div>, <div>before{}after</div>, <>before{}after</>, <>before{}after</>, <>before{}after</>);
//#endregion

```
### diff
```diff
===================================================================
--- esbuild	/out/entry.js
+++ rolldown	entry.js
@@ -1,46 +1,3 @@
-// entry.jsx
-console.log(
-  <div x={
-    /*before*/
-    x
-  } />,
-  <div x={
-    /*before*/
-    "y"
-  } />,
-  <div x={
-    /*before*/
-    true
-  } />,
-  <div {
-    /*before*/
-    ...x
-  } />,
-  <div>{
-    /*before*/
-    x
-  }</div>,
-  <>{
-    /*before*/
-    x
-  }</>,
-  // Comments on absent AST nodes
-  <div>before{}after</div>,
-  <div>before{
-    /* comment 1 */
-    /* comment 2 */
-  }after</div>,
-  <div>before{
-    // comment 1
-    // comment 2
-  }after</div>,
-  <>before{}after</>,
-  <>before{
-    /* comment 1 */
-    /* comment 2 */
-  }after</>,
-  <>before{
-    // comment 1
-    // comment 2
-  }after</>
-);
\ No newline at end of file
+//#region entry.jsx
+console.log(<div x={x} />, <div x={"y"} />, <div x={true} />, <div {...x} />, <div>{x}</div>, <>{x}</>, <div>before{}after</div>, <div>before{}after</div>, <div>before{}after</div>, <>before{}after</>, <>before{}after</>, <>before{}after</>);
+//#endregion

```