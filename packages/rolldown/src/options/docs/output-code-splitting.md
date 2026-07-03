:::warning

Be aware that manual code splitting can change the behavior of the application if side effects are triggered before the corresponding modules are actually used. You can change the chunking configuration to keep order-sensitive modules together, or you can use the [`output.strictExecutionOrder`](https://rolldown.rs/reference/OutputOptions.strictExecutionOrder) option to preserve source execution order. The option wraps only modules that are at risk in the generated chunk graph, and its bundle-size cost depends on how many modules need that wrapping.

:::
