# Already-loaded side-effectful barrel

This fixture models a package barrel with a one-time side effect and independent
pure module families.

## Graph

- `entry.js` eagerly uses two families from `library/index.js`: one directly
  and one through `eager-consumer.js`.
- The entry dynamically imports three consumers.
- Each lazy consumer imports a different pure family from the same barrel.
- One exported family is unused and should be eliminated.

## Expected layout

Default code splitting should emit four files:

```text
entry.js
lazy-consumer-a.js
lazy-consumer-b.js
lazy-consumer-c.js
```

The barrel's side effect belongs in `entry.js`, which is evaluated before any
lazy consumer. Each pure family stays with its consumer, and the unused family
is removed.
