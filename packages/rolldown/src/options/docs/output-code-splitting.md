:::warning

Be aware that manual code splitting can change the behavior of the application if side effects are triggered before the corresponding modules are actually used. You can change the chunking configuration to group some modules so that the modules are reordered, or you can use the [`output.strictExecutionOrder`](https://rolldown.rs/reference/OutputOptions.strictExecutionOrder) option to ensure that modules are executed in the order they are imported with the cost of a slight increase in bundle size.

:::
