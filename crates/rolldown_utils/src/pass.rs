//! Compile-time contract for synchronous pipeline passes.
//!
//! A pass declares shared reads, owned inputs, a newly minted read-only output,
//! and owned outputs. Callers must execute passes through [`run_pass`] or
//! [`run_infallible_pass`]. The harness brands each invocation, records
//! diagnostic provenance, and seals the minted output before returning it.

#![forbid(unsafe_code)]

use std::{any::type_name, convert::Infallible, marker::PhantomData, mem::size_of, ops::Deref};

use rolldown_error::{BuildDiagnostic, Diagnostics};

/// One synchronous top-level pipeline step.
///
/// Pass values are names, not runtime state. [`run_pass`] rejects non-zero-sized
/// values during code generation. Configuration and pipeline data belong in the
/// declared slots.
pub trait Pass: Sized + Copy + 'static {
  /// Shared inputs. `Copy` rules out `&mut` directly and in ordinary manifests.
  type InputRead<'a>: Copy;
  /// Inputs whose ownership moves into the pass.
  type InputOwned: 'static;
  /// A purpose-specific fact minted by this pass and sealed by the harness.
  type OutputRead: 'static;
  /// Still-mutable data whose ownership moves to the next driver step.
  type OutputOwned: 'static;
  /// The pass-specific failure channel. Infallible pipelines use [`Infallible`].
  type Error: 'static;

  /// Performs the pass body after receiving a capability for this invocation.
  ///
  /// The raw output can only be constructed by consuming `token` through
  /// [`RunToken::finish`]. Its fields remain private to this module, so the
  /// harness is the only code that can open it and expose a [`Sealed`] result.
  fn run(
    self,
    token: RunToken<'_, Self>,
    cx: &mut PassCtx<'_>,
    read: Self::InputRead<'_>,
    owned: Self::InputOwned,
  ) -> Result<RawPassOutput<Self::OutputRead, Self::OutputOwned>, Self::Error>;
}

struct RunBrand;

/// Invocation capability minted by the pass harness.
///
/// The private, invariant brand prevents safe reachable code from constructing
/// or retaining this token after the invocation. It intentionally implements
/// neither `Copy` nor `Clone`.
pub struct RunToken<'run, P> {
  _brand: &'run mut RunBrand,
  _lifetime: PhantomData<fn(&'run mut ()) -> &'run mut ()>,
  _pass: PhantomData<fn(P) -> P>,
}

impl<P> RunToken<'_, P> {
  /// Finishes one pass invocation and consumes its capability.
  pub fn finish<R, O>(self, read: R, owned: O) -> RawPassOutput<R, O> {
    RawPassOutput { read, owned }
  }
}

/// Opaque result returned from [`Pass::run`] to the harness.
pub struct RawPassOutput<R, O> {
  read: R,
  owned: O,
}

/// A minted artifact for which the pipeline exposes shared access only.
///
/// This wrapper has no public constructor, mutable dereference, or unwrap. A
/// draft/final artifact pair is still required when domain finalization should
/// also narrow the underlying API or representation.
pub struct Sealed<T>(T);

impl<T> Sealed<T> {
  fn new(value: T) -> Self {
    Self(value)
  }
}

impl<T> Deref for Sealed<T> {
  type Target = T;

  fn deref(&self) -> &Self::Target {
    &self.0
  }
}

/// Public output of a pass after the harness seals its minted artifact.
pub type PassOutput<P> = (Sealed<<P as Pass>::OutputRead>, <P as Pass>::OutputOwned);

/// Result of a fallible pass after the harness applies its output contract.
pub type PassResult<P> = Result<PassOutput<P>, <P as Pass>::Error>;

struct EmittedDiagnostic {
  pass: &'static str,
  diagnostic: BuildDiagnostic,
}

/// Driver-owned diagnostic sink for one serial pipeline or parallel branch.
///
/// A concurrent driver gives each branch its own context and appends completed
/// contexts in declared pass order.
#[derive(Default)]
pub struct PassPipelineCtx {
  diagnostics: Vec<EmittedDiagnostic>,
}

impl PassPipelineCtx {
  /// Creates an empty pipeline context.
  pub fn new() -> Self {
    Self::default()
  }

  /// Appends a completed branch while preserving both local and branch order.
  pub fn append(&mut self, other: Self) {
    self.diagnostics.extend(other.diagnostics);
  }

  /// Consumes the write-only pipeline sink and returns ordinary diagnostics.
  pub fn into_diagnostics(self) -> Diagnostics {
    Diagnostics::from(
      self
        .diagnostics
        .into_iter()
        .map(|emission| {
          tracing::trace!(
            target: "rolldown::pass",
            pass = emission.pass,
            diagnostic_kind = ?emission.diagnostic.kind(),
            "pass diagnostic"
          );
          emission.diagnostic
        })
        .collect::<Vec<_>>(),
    )
  }
}

/// Write-only view handed to one pass invocation.
///
/// It deliberately has no constructor, getters, drain method, `Default`, or
/// access to the driver-owned [`PassPipelineCtx`].
pub struct PassCtx<'ctx> {
  pipeline: &'ctx mut PassPipelineCtx,
  pass: &'static str,
}

impl PassCtx<'_> {
  /// Emits one diagnostic with automatic pass provenance.
  pub fn push(&mut self, diagnostic: BuildDiagnostic) {
    self.pipeline.diagnostics.push(EmittedDiagnostic { pass: self.pass, diagnostic });
  }

  /// Emits diagnostics in iterator order with automatic pass provenance.
  pub fn extend(&mut self, diagnostics: impl IntoIterator<Item = BuildDiagnostic>) {
    for diagnostic in diagnostics {
      self.push(diagnostic);
    }
  }
}

/// Runs one pass, records its span and diagnostic provenance, and seals its
/// minted read-side output.
pub fn run_pass<P: Pass>(
  pass: P,
  pipeline: &mut PassPipelineCtx,
  read: P::InputRead<'_>,
  owned: P::InputOwned,
) -> PassResult<P> {
  const {
    assert!(size_of::<P>() == 0, "a pass is a name; runtime state belongs in declared slots");
  }

  let pass_name = type_name::<P>();
  let span = tracing::debug_span!(target: "rolldown::pass", "run_pass", pass = pass_name);

  span.in_scope(|| {
    let mut brand = RunBrand;
    let token = RunToken { _brand: &mut brand, _lifetime: PhantomData, _pass: PhantomData };
    let mut cx = PassCtx { pipeline, pass: pass_name };

    let raw = match pass.run(token, &mut cx, read, owned) {
      Ok(raw) => raw,
      Err(error) => return Err(error),
    };
    let RawPassOutput { read, owned } = raw;
    Ok((Sealed::new(read), owned))
  })
}

/// Runs a pass whose error type is uninhabited without exposing a `Result`.
pub fn run_infallible_pass<P: Pass<Error = Infallible>>(
  pass: P,
  pipeline: &mut PassPipelineCtx,
  read: P::InputRead<'_>,
  owned: P::InputOwned,
) -> PassOutput<P> {
  match run_pass(pass, pipeline, read, owned) {
    Ok(output) => output,
    Err(never) => match never {},
  }
}
