### A base TSConfig for working with Node 20.

Add the package to your `"devDependencies"`:

```sh
npm install --save-dev @tsconfig/node20
yarn add --dev @tsconfig/node20
```

Add to your `tsconfig.json`:

```json
"extends": "@tsconfig/node20/tsconfig.json"
```

---

The `tsconfig.json`: 

```jsonc
{
  "$schema": "https://www.schemastore.org/tsconfig",
  "_version": "20.1.0",

  "compilerOptions": {
    "lib": ["es2023"],
    "module": "nodenext",
    "target": "es2022",

    "strict": true,
    "esModuleInterop": true,
    "skipLibCheck": true,
    "moduleResolution": "node16"
  }
}

```

You can find the [code here](https://github.com/tsconfig/bases/blob/master/bases/node20.json).
