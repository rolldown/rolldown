##### In-depth

Each chunk will include a comment explaining the reason why it was created:

| Reason                | Format                                                                 | Description                                                                                       |
| --------------------- | ---------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------- |
| User-defined Entry    | `User-defined Entry: [Entry-Module-Id: <path>] [Name: Some("<name>")]` | Explicit entry point from build config                                                            |
| Dynamic Entry         | `Dynamic Entry: [Entry-Module-Id: <path>] [Name: None]`                | Chunk created from `import()` expression                                                          |
| Common Chunk          | `Common Chunk: [Shared-By: <entry1>, <entry2>, ...]`                   | Shared modules extracted for multiple entries                                                     |
| Manual Code Splitting | `ManualCodeSplitting: [Group-Name: <name>]`                            | Chunk created by [`output.codeSplitting`](/reference/OutputOptions.codeSplitting) option          |
| Preserve Modules      | `Enabling Preserve Module: [User-defined: <bool>] [Module-Id: <path>]` | Per-module chunk from [`output.preserveModules`](/reference/OutputOptions.preserveModules) option |

When rolldown optimized away empty facade chunks (entry chunks with no modules of their own), the target chunk will include `Eliminated Facade Chunk: [Chunk-Name: <name>] [Entry-Module-Id: <path>]`.
