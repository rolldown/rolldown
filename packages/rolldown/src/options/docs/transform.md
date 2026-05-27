#### In-depth

Rolldown uses Oxc under the hood for transformation.

While Oxc does not support lowering the latest decorators proposal yet, Rolldown is able to bundle them.

##### Legacy decorator metadata

When `transform.decorator.emitDecoratorMetadata` is enabled, the `design:type` metadata emitted for a nullable union such as `string | null` defaults to `Object`, matching `tsc` with `strictNullChecks` enabled.

Set `transform.decorator.strictNullChecks` to `false` to elide `null` and `undefined` from the union and emit the underlying primitive constructor instead. This matches `tsc --strictNullChecks false` and `babel-plugin-transform-typescript-metadata`, which libraries such as NestJS, class-validator, and TypeORM rely on. It defaults to `true`.

```ts
class User {
  @Column()
  name: string | null;
}
```

```js
// `design:type` recorded for `name`:
//   strictNullChecks: true  (default)  ->  Object
//   strictNullChecks: false            ->  String
```

:::tip
`strictNullChecks` is **not** inferred from `tsconfig.json` — unlike `experimentalDecorators` and `emitDecoratorMetadata`, it must be set explicitly on `transform.decorator`.
:::

See [Oxc Transformer's document](https://oxc.rs/docs/guide/usage/transformer) for more details.
