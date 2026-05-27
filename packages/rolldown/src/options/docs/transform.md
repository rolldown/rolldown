#### In-depth

Rolldown uses Oxc under the hood for transformation.

While Oxc does not support lowering the latest decorators proposal yet, Rolldown is able to bundle them.

##### Legacy decorator metadata

When `transform.decorator.emitDecoratorMetadata` is enabled, the emitted `design:type` metadata for a nullable
union such as `string | null` defaults to `Object`, matching `tsc` with `strictNullChecks` turned on.

Set `transform.decorator.strictNullChecks` to `false` to elide `null` and `undefined` from the union and emit the
underlying constructor instead (for example `string | null` becomes `String`). This matches
`tsc --strictNullChecks false` and `babel-plugin-transform-typescript-metadata`, which some libraries (NestJS,
class-validator, TypeORM) rely on. It defaults to `true`.

> Note: `strictNullChecks` is **not** inferred from `tsconfig.json`; set it explicitly on `transform.decorator`.

See [Oxc Transformer's document](https://oxc.rs/docs/guide/usage/transformer) for more details.
