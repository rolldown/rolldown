# Docs

Rolldown is documented with [doc-kit](https://github.com/nodejs/doc-kit), the documentation generator of Node.js. You can find the source code for the site in `docs`. Pages are Markdown (or MDX, when they use components) following [doc-kit's specification](https://github.com/nodejs/doc-kit/blob/main/docs/specification.md).

To contribute to the documentation, you can start the docs dev server running on the project root:

```sh
pnpm run docs
```

Since the `pnpm docs` command is used for opening the module introduction in `npm`, you may use the command above.

You can then edit the Markdown files: the site rebuilds, and you can refresh the page to see your changes. The navigation is configured at `docs/theme/site.mjs`, the build at `docs/doc-kit.config.mjs`, and the theme's components and styles live in `docs/theme`. The API reference is generated from the TypeScript sources of `packages/rolldown` by TypeDoc, with doc-kit's TypeDoc plugin, as configured at `docs/typedoc.config.mjs`.

If you'd like to review the built site, run in the project root:

```sh
pnpm docs:build
pnpm docs:preview
```

This step isn't needed when contributing if you aren't modifying the docs build setup.
