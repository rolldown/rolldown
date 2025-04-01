# Reason
1. trivial codegen diff, esbuild will try to join multiple `varDeclaration`
# Diff
## /out.js
### esbuild
```js
// entry.ts
var A_keep = class {
  static {
    foo;
  }
}, B_keep = class {
  static {
    this.foo;
  }
}, C_keep = class {
  static {
    try {
      foo;
    } catch {
    }
  }
}, D_keep = class {
  static {
    try {
    } finally {
      foo;
    }
  }
};
```
### rolldown
```js

//#region entry.ts
var A_keep = class {
	static {
		foo;
	}
};
var B_keep = class {
	static {
		this.foo;
	}
};
var C_keep = class {
	static {
		try {
			foo;
		} catch {}
	}
};
var D_keep = class {
	static {
		{
			foo;
		}
	}
};
//#endregion

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,21 +1,24 @@
 var A_keep = class {
     static {
         foo;
     }
-}, B_keep = class {
+};
+var B_keep = class {
     static {
         this.foo;
     }
-}, C_keep = class {
+};
+var C_keep = class {
     static {
         try {
             foo;
         } catch {}
     }
-}, D_keep = class {
+};
+var D_keep = class {
     static {
-        try {} finally {
+        {
             foo;
         }
     }
 };

```