# Docs

Rolldown is documented using [VitePress](https://vitepress.dev). You can find the source code for the site in `docs`. Check out the [Markdown Extensions Guide](https://vitepress.dev/guide/markdown) to learn about VitePress features.

To contribute to the documentation, you can start the docs dev server running on the project root:

```sh
pnpm run docs
```

Since the `pnpm docs` command is used for opening the module introduction in `npm`, you may use the command above.

You can then edit the markdown files and see your changes instantly. The docs structure is configured at `docs/.vitepress/config.ts` (see the [Site Config Reference](https://vitepress.dev/reference/site-config)).

If you'd like to review the built site, run in the project root:

```sh
pnpm docs:build
pnpm docs:preview
```

This step isn't needed when contributing if you aren't modifying the docs build setup.
