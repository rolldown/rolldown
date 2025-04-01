# Reason
1. rolldown don't have metafile, rest part are same
# Diff
## entry.js.map
### esbuild
```js
{
  "version": 3,
  "sources": ["entry.js"],
  "sourcesContent": ["\n\t\t\t\timport './a.empty'\n\t\t\t\timport * as ns from './b.empty'\n\t\t\t\timport def from './c.empty'\n\t\t\t\timport { named } from './d.empty'\n\t\t\t\tconsole.log(ns, def, named)\n\t\t\t"],
  "mappings": ";;;;;;;;;;;;;AAEI,SAAoB;AACpB,eAAgB;AAEhB,QAAQ,IAAI,IAAI,SAAAA,SAAK,MAAK;",
  "names": ["def"]
}
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	entry.js.map
+++ rolldown	
@@ -1,7 +0,0 @@
-{
-  "version": 3,
-  "sources": ["entry.js"],
-  "sourcesContent": ["\n\t\t\t\timport './a.empty'\n\t\t\t\timport * as ns from './b.empty'\n\t\t\t\timport def from './c.empty'\n\t\t\t\timport { named } from './d.empty'\n\t\t\t\tconsole.log(ns, def, named)\n\t\t\t"],
-  "mappings": ";;;;;;;;;;;;;AAEI,SAAoB;AACpB,eAAgB;AAEhB,QAAQ,IAAI,IAAI,SAAAA,SAAK,MAAK;",
-  "names": ["def"]
-}
\ No newline at end of file

```
## entry.js
### esbuild
```js
// b.empty
var require_b = __commonJS({
  "b.empty"() {
  }
});

// c.empty
var require_c = __commonJS({
  "c.empty"() {
  }
});

// entry.js
var ns = __toESM(require_b());
var import_c = __toESM(require_c());
console.log(ns, import_c.default, void 0);
```
### rolldown
```js
import assert from "node:assert";

//#region b.empty
var b_exports = {};

//#endregion

//#region c.empty
var default$1 = void 0;

//#endregion

//#region d.empty
var named = void 0;

//#endregion

//#region entry.js
assert.deepEqual(b_exports, {});
assert.deepEqual(default$1, void 0);
assert.equal(named, void 0);

//#endregion

//# sourceMappingURL=entry.js.map
```
### diff
```diff
===================================================================
--- esbuild	entry.js
+++ rolldown	entry.js
@@ -1,9 +1,4 @@
-var require_b = __commonJS({
-    "b.empty"() {}
-});
-var require_c = __commonJS({
-    "c.empty"() {}
-});
-var ns = __toESM(require_b());
-var import_c = __toESM(require_c());
-console.log(ns, import_c.default, void 0);
+var b_exports = {};
+var default$1 = void 0;
+var named = void 0;
+console.log(b_exports, default$1, named);

```
## metafile.json
### esbuild
```js
{
  "inputs": {
    "a.empty": {
      "bytes": 0,
      "imports": []
    },
    "b.empty": {
      "bytes": 0,
      "imports": []
    },
    "c.empty": {
      "bytes": 0,
      "imports": []
    },
    "d.empty": {
      "bytes": 0,
      "imports": []
    },
    "entry.js": {
      "bytes": 165,
      "imports": [
        {
          "path": "a.empty",
          "kind": "import-statement",
          "original": "./a.empty"
        },
        {
          "path": "b.empty",
          "kind": "import-statement",
          "original": "./b.empty"
        },
        {
          "path": "c.empty",
          "kind": "import-statement",
          "original": "./c.empty"
        },
        {
          "path": "d.empty",
          "kind": "import-statement",
          "original": "./d.empty"
        }
      ],
      "format": "esm"
    }
  },
  "outputs": {
    "entry.js.map": {
      "imports": [],
      "exports": [],
      "inputs": {},
      "bytes": 377
    },
    "entry.js": {
      "imports": [],
      "exports": [],
      "entryPoint": "entry.js",
      "inputs": {
        "b.empty": {
          "bytesInOutput": 53
        },
        "c.empty": {
          "bytesInOutput": 53
        },
        "entry.js": {
          "bytesInOutput": 111
        }
      },
      "bytes": 253
    }
  }
}
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	metafile.json
+++ rolldown	
@@ -1,71 +0,0 @@
-{
-  "inputs": {
-    "a.empty": {
-      "bytes": 0,
-      "imports": []
-    },
-    "b.empty": {
-      "bytes": 0,
-      "imports": []
-    },
-    "c.empty": {
-      "bytes": 0,
-      "imports": []
-    },
-    "d.empty": {
-      "bytes": 0,
-      "imports": []
-    },
-    "entry.js": {
-      "bytes": 165,
-      "imports": [
-        {
-          "path": "a.empty",
-          "kind": "import-statement",
-          "original": "./a.empty"
-        },
-        {
-          "path": "b.empty",
-          "kind": "import-statement",
-          "original": "./b.empty"
-        },
-        {
-          "path": "c.empty",
-          "kind": "import-statement",
-          "original": "./c.empty"
-        },
-        {
-          "path": "d.empty",
-          "kind": "import-statement",
-          "original": "./d.empty"
-        }
-      ],
-      "format": "esm"
-    }
-  },
-  "outputs": {
-    "entry.js.map": {
-      "imports": [],
-      "exports": [],
-      "inputs": {},
-      "bytes": 377
-    },
-    "entry.js": {
-      "imports": [],
-      "exports": [],
-      "entryPoint": "entry.js",
-      "inputs": {
-        "b.empty": {
-          "bytesInOutput": 53
-        },
-        "c.empty": {
-          "bytesInOutput": 53
-        },
-        "entry.js": {
-          "bytesInOutput": 111
-        }
-      },
-      "bytes": 253
-    }
-  }
-}
\ No newline at end of file

```