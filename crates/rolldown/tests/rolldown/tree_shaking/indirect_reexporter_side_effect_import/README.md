# Indirect re-exporter side-effect import

A used indirect re-export executes the barrel, including its bare import of a module explicitly listed in the package's `sideEffects` allowlist. This remains true with on-demand wrapping enabled.
