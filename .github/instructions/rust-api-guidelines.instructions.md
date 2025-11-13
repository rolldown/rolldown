---
applyTo: '**/*.rs'
excludeAgent: ['coding-agent']
---

# Rust API Guidelines - Copilot Review Instructions

When reviewing Rust code changes, check if the code adheres to the Rust API Guidelines. These guidelines are advisory recommendations from the Rust library team for designing consistent, ergonomic APIs.

## Motivation

- **Idiomatic APIs** — Helps your crate match common Rust design patterns.
- **Better Interoperability** — Ensures smoother integration with the Rust ecosystem.
- **Proven Best Practices** — Based on real experience from the Rust library team.
- **Helpful Guidance** — Not mandatory, but leads to cleaner and more consistent APIs.
- **Shared Mental Models** — When everyone designs APIs similarly, others can understand the code faster.

## Context

- `crates/rolldown_binding` is quite special as it bridges Rust and TypeScript, so ignore rust-api-guidelines checks there.

## Workflow

**IMPORTANT: Batch your checks efficiently. Do NOT go through guidelines one by one.**

When reviewing code changes:

1. **Identify ALL applicable scenarios** — Scan the code changes and determine which guideline categories apply (e.g., new type definitions, function signatures, trait definitions, etc.)

2. **Group related checks together** — Collect all the relevant guideline sections that need to be reviewed for the changes at hand

3. **Visit ALL needed links in parallel** — Open and review ALL applicable guideline URLs in one go, rather than visiting them sequentially one-by-one

4. **Provide comprehensive feedback** — After reviewing all applicable guidelines together, provide complete, batched feedback on all relevant items

This batched approach is significantly more efficient than checking guidelines sequentially.

### When reviewing TYPE, TRAIT, FUNCTION, or VARIABLE NAMES

Check: Naming Guidelines - https://rust-lang.github.io/api-guidelines/naming.html

- Verify casing follows RFC 430 (UpperCamelCase for types/traits, snake_case for functions/variables, SCREAMING_SNAKE_CASE for constants)
- Conversion methods follow `as_`, `to_`, `into_` conventions (C-CONV)
- Getter methods don't use `get_` prefix unless necessary (C-GETTER)
- Iterator methods follow `iter`, `iter_mut`, `into_iter` pattern (C-ITER)
- Iterator type names match their producing methods (C-ITER-TY)
- Feature names in Cargo.toml are meaningful, not placeholders (C-FEATURE)
- Related names use consistent word ordering (C-WORD-ORDER)

### When reviewing NEW TYPE DEFINITIONS (struct, enum, union)

Check: Interoperability Guidelines - https://rust-lang.github.io/api-guidelines/interoperability.html

- Implements common traits: `Clone`, `Debug`, `PartialEq`, etc. where appropriate (C-COMMON-TRAITS)
- Provides `From`/`TryFrom` for fallible conversions (C-CONV-TRAITS)
- Implements `Send` and `Sync` where possible for thread safety (C-SEND-SYNC)
- For binary types, implements binary formatting traits (C-NUM-FMT)

Check: Debuggability Guidelines - https://rust-lang.github.io/api-guidelines/debuggability.html

- All public types implement `Debug` (C-DEBUG)
- Debug output is never empty, even for empty collections (C-DEBUG-NONEMPTY)

Check: Type Safety Guidelines - https://rust-lang.github.io/api-guidelines/type-safety.html

- Uses newtype pattern for semantically distinct values with same underlying type (C-NEWTYPE)
- Avoids primitive/bool obsession in favor of descriptive types (C-CUSTOM-TYPE)

Check: Future Proofing Guidelines - https://rust-lang.github.io/api-guidelines/future-proofing.html

- Struct fields are private to maintain flexibility (C-STRUCT-PRIVATE)
- Generic struct derives don't duplicate bounds already on struct definition (C-STRUCT-BOUNDS)

### When reviewing TRAIT DEFINITIONS

Check: Interoperability Guidelines - https://rust-lang.github.io/api-guidelines/interoperability.html

- Error types are meaningful and implement `std::error::Error` (C-GOOD-ERR)

Check: Flexibility Guidelines - https://rust-lang.github.io/api-guidelines/flexibility.html

- Traits that may be used as trait objects are object-safe (C-OBJECT)

Check: Future Proofing Guidelines - https://rust-lang.github.io/api-guidelines/future-proofing.html

- Consider sealing traits that shouldn't be implemented downstream (C-SEALED)

### When reviewing COLLECTION TYPE IMPLEMENTATIONS

Check: Interoperability Guidelines - https://rust-lang.github.io/api-guidelines/interoperability.html

- Implements `FromIterator` and `Extend` for collection types (C-COLLECT)

### When reviewing DATA STRUCTURES for SERIALIZATION

Check: Interoperability Guidelines - https://rust-lang.github.io/api-guidelines/interoperability.html

- Implements Serde's `Serialize` and `Deserialize` traits (C-SERDE)

### When reviewing CONVERSION METHOD implementations

Check: Naming Guidelines - https://rust-lang.github.io/api-guidelines/naming.html

- `as_` for cheap reference-to-reference conversions
- `to_` for expensive conversions
- `into_` for consuming conversions

Check: Interoperability Guidelines - https://rust-lang.github.io/api-guidelines/interoperability.html

- Uses `From`, `TryFrom`, `AsRef`, `AsMut` traits (C-CONV-TRAITS)

Check: Predictability Guidelines - https://rust-lang.github.io/api-guidelines/predictability.html

- Conversions live on the most specific type involved (C-CONV-SPECIFIC)

### When reviewing FUNCTION SIGNATURES

Check: Flexibility Guidelines - https://rust-lang.github.io/api-guidelines/flexibility.html

- Functions expose intermediate results to avoid duplicate work (C-INTERMEDIATE)
- Caller controls data placement (avoid unnecessary clones) (C-CALLER-CONTROL)
- Uses generics to minimize assumptions (accept `IntoIterator`, not `Vec`) (C-GENERIC)

Check: Interoperability Guidelines - https://rust-lang.github.io/api-guidelines/interoperability.html

- Reader/writer functions take `R: Read` or `W: Write` by value (C-RW-VALUE)

Check: Predictability Guidelines - https://rust-lang.github.io/api-guidelines/predictability.html

- Functions with clear receiver are methods, not free functions (C-METHOD)
- Functions don't use out-parameters (prefer tuples or custom return types) (C-NO-OUT)

Check: Type Safety Guidelines - https://rust-lang.github.io/api-guidelines/type-safety.html

- Parameters convey meaning through types, not `bool` or `Option` where custom types would be clearer (C-CUSTOM-TYPE)

Check: Dependability Guidelines - https://rust-lang.github.io/api-guidelines/dependability.html

- Functions validate their arguments appropriately (C-VALIDATE)

### When reviewing CONSTRUCTOR or FACTORY METHODS

Check: Predictability Guidelines - https://rust-lang.github.io/api-guidelines/predictability.html

- Constructors are static, inherent methods (usually `new` or `with_*`) (C-CTOR)

Check: Type Safety Guidelines - https://rust-lang.github.io/api-guidelines/type-safety.html

- Complex construction uses builder pattern (C-BUILDER)

### When reviewing SMART POINTER types (Box-like, Rc-like, Arc-like)

Check: Predictability Guidelines - https://rust-lang.github.io/api-guidelines/predictability.html

- Smart pointers don't add inherent methods (methods go on the pointee type) (C-SMART-PTR)
- Only smart pointers implement `Deref` and `DerefMut` (C-DEREF)

### When reviewing OPERATOR TRAIT implementations (Add, Sub, Mul, Div, etc.)

Check: Predictability Guidelines - https://rust-lang.github.io/api-guidelines/predictability.html

- Operator overloads are unsurprising and match mathematical expectations (C-OVERLOAD)

### When reviewing FLAG or OPTION handling

Check: Type Safety Guidelines - https://rust-lang.github.io/api-guidelines/type-safety.html

- C-like flags use `bitflags!` crate instead of enums (C-BITFLAG)

### When reviewing DROP implementations

Check: Dependability Guidelines - https://rust-lang.github.io/api-guidelines/dependability.html

- Destructors never fail (panic or return errors) (C-DTOR-FAIL)
- Destructors that may block have non-blocking alternatives (C-DTOR-BLOCK)

### When reviewing MACRO definitions (declarative or procedural)

Check: Macro Guidelines - https://rust-lang.github.io/api-guidelines/macros.html

- Input syntax evokes the output (C-EVOCATIVE)
- Item macros compose with attributes like `#[derive]`, `#[cfg]` (C-MACRO-ATTR)
- Item macros work anywhere items are allowed (C-ANYWHERE)
- Item macros support visibility specifiers (`pub`, `pub(crate)`, etc.) (C-MACRO-VIS)
- Type fragments are flexible (don't over-constrain) (C-MACRO-TY)
