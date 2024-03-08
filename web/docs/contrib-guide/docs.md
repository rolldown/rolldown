# Docs

Rolldown is documented using [VitePress](https://vitepress.dev). You can find the source code for the site in `web/docs`. Check out the [Markdown Extensions Guide](https://vitepress.dev/guide/markdown) to learn about VitePress features.

To contribute to the documentation, you can start the docs dev server running on the project root:

```
yarn docs
```

You can then edit the markdown files and see your changes instantly. The docs structure is configured at `web/docs/.vitepress/config.ts` (see the [Site Config Reference](https://vitepress.dev/reference/site-config)).

If you'd like to review the built site, run in the project root:

```
yarn docs:build
yarn docs:preview
```

This step isn't needed when contributing if you aren't modifying the docs build setup.
