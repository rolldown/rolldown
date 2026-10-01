# Default indirect re-exporter side effects

Used indirect re-exporters must retain their body effects across named-to-default and default-to-named forwarding. A direct first hop remains transparent and does not retain its body.

This mirrors Rollup's `respect-default-export-reexporter-side-effects` tree-shaking test.
