:::warning

Be aware that manual code splitting can change the behavior of the application if side effects are triggered before the corresponding modules are actually used. You can change the chunking configuration to keep order-sensitive modules together, or you can use the [`output.strictExecutionOrder`](https://rolldown.rs/reference/OutputOptions.strictExecutionOrder) option to preserve source execution order. The option wraps modules so their bodies run in source order, at a bundle-size cost; `experimental.onDemandWrapping` replaces wrap-all with a conservative plan derived from predicted chunk execution hazards.

:::
