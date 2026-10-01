# Symbol-sensitive barrel reachability

This fixture models a package whose public barrel imports a one-time
configuration module and re-exports independent module families. Three lazy
consumers import disjoint named exports from that barrel.

## Graph

```text
entry
  ├─ import() light-consumer-a ─┐
  ├─ import() light-consumer-b ─┼─ library/index
  └─ import() heavy-consumer ───┘       ├─ configure (side effect)
                                        ├─ light-a
                                        ├─ light-b
                                        └─ heavy
```

All three exported families are live somewhere in the bundle, but each lazy
consumer uses exactly one family. Importing any family must execute
`configure.js` exactly once.

`configure.js` is the only module listed in the package's `sideEffects`
allowlist. The barrel and three families are declared side-effect-free on
purpose: a family whose _evaluation_ has an effect rolldown cannot prove away
has to run whenever the barrel runs, so it belongs in every consumer's closure
and none of the exclusions below would hold for it. Keep the family modules free
of top-level calls.

## Expected layout

Reachability should follow the bindings retained for each consumer, not the
barrel's bundle-wide dependency union:

- every consumer's static closure includes `configure.js`;
- the light-A closure excludes light-B and heavy code;
- the light-B closure excludes light-A and heavy code;
- the heavy closure excludes both light families.

A small shared configuration chunk is valid. Co-locating an unrelated family
with that shared configuration is not: it makes loading either light consumer
also load the heavy family.

The runtime assertions run first to pin side-effect semantics. Every emitted
chunk must be reachable from the entry or a dynamic entry. The default and
strict on-demand variants then compare each dynamic entry's static closure by
using unique marker strings. The preserve-modules variant pins runtime and
reachability semantics but intentionally retains the barrel's original imports,
so it does not apply the family-exclusion assertions.

This is distinct from `already_loaded_side_effectful_barrel`. In that fixture,
the root entry eagerly executes the barrel before every lazy consumer, so the
already-loaded reduction can fold it. Here the barrel is first reached by
independent lazy consumers; no one consumer already loads the other consumers'
exports, so the layout rests on symbol-sensitive dependent-entry assignment.
`load_dependencies` keeps unrelated side-effect-free importees out of each
consumer's reachability set, while
`indirect_reexport_load_dependencies` records the specific barrel execution
edge for each used binding. Cross-chunk linking preserves that edge even though
the package declares the barrel itself side-effect-free.
