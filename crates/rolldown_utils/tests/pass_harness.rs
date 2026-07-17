use std::{
  any::type_name,
  convert::Infallible,
  sync::{Arc, Mutex, PoisonError},
};

use rolldown_error::BuildDiagnostic;
use rolldown_utils::pass::{
  Pass, PassCtx, PassPipelineCtx, RawPassOutput, RunToken, run_infallible_pass, run_pass,
};
use tracing::{Event, Subscriber, field::Visit, span::Attributes};
use tracing_subscriber::{Layer, layer::Context, prelude::*};

#[derive(Clone, Debug, PartialEq, Eq)]
struct SpanRecord {
  name: &'static str,
  target: &'static str,
  pass: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct EventRecord {
  target: &'static str,
  pass: Option<String>,
}

#[derive(Clone, Default)]
struct RecordingLayer {
  spans: Arc<Mutex<Vec<SpanRecord>>>,
  events: Arc<Mutex<Vec<EventRecord>>>,
}

impl RecordingLayer {
  fn spans(&self) -> Vec<SpanRecord> {
    self.spans.lock().unwrap_or_else(PoisonError::into_inner).clone()
  }

  fn events(&self) -> Vec<EventRecord> {
    self.events.lock().unwrap_or_else(PoisonError::into_inner).clone()
  }
}

#[derive(Default)]
struct PassFieldVisitor {
  pass: Option<String>,
}

impl Visit for PassFieldVisitor {
  fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
    if field.name() == "pass" {
      self.pass = Some(format!("{value:?}"));
    }
  }

  fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
    if field.name() == "pass" {
      self.pass = Some(value.to_string());
    }
  }
}

impl<S: Subscriber> Layer<S> for RecordingLayer {
  fn on_new_span(&self, attrs: &Attributes<'_>, _id: &tracing::Id, _ctx: Context<'_, S>) {
    let mut visitor = PassFieldVisitor::default();
    attrs.record(&mut visitor);
    let metadata = attrs.metadata();
    self.spans.lock().unwrap_or_else(PoisonError::into_inner).push(SpanRecord {
      name: metadata.name(),
      target: metadata.target(),
      pass: visitor.pass,
    });
  }

  fn on_event(&self, event: &Event<'_>, _ctx: Context<'_, S>) {
    let mut visitor = PassFieldVisitor::default();
    event.record(&mut visitor);
    self
      .events
      .lock()
      .unwrap_or_else(PoisonError::into_inner)
      .push(EventRecord { target: event.metadata().target(), pass: visitor.pass });
  }
}

#[derive(Clone, Copy)]
struct EmitPass;

impl Pass for EmitPass {
  type InputRead<'a> = &'a str;
  type InputOwned = u32;
  type OutputRead = usize;
  type OutputOwned = u32;
  type Error = Infallible;

  fn run(
    self,
    token: RunToken<'_, Self>,
    cx: &mut PassCtx<'_>,
    read: Self::InputRead<'_>,
    owned: Self::InputOwned,
  ) -> Result<RawPassOutput<Self::OutputRead, Self::OutputOwned>, Self::Error> {
    cx.push(BuildDiagnostic::bundler_initialize_error(read.to_string(), None));
    Ok(token.finish(read.len(), owned + 1))
  }
}

#[derive(Clone, Copy)]
struct ConsumeOwnedPass;

impl Pass for ConsumeOwnedPass {
  type InputRead<'a> = ();
  type InputOwned = u32;
  type OutputRead = u32;
  type OutputOwned = ();
  type Error = Infallible;

  fn run(
    self,
    token: RunToken<'_, Self>,
    _cx: &mut PassCtx<'_>,
    (): Self::InputRead<'_>,
    owned: Self::InputOwned,
  ) -> Result<RawPassOutput<Self::OutputRead, Self::OutputOwned>, Self::Error> {
    Ok(token.finish(owned * 2, ()))
  }
}

#[derive(Clone, Copy)]
struct SecondEmitPass;

impl Pass for SecondEmitPass {
  type InputRead<'a> = &'a str;
  type InputOwned = ();
  type OutputRead = ();
  type OutputOwned = ();
  type Error = Infallible;

  fn run(
    self,
    token: RunToken<'_, Self>,
    cx: &mut PassCtx<'_>,
    read: Self::InputRead<'_>,
    (): Self::InputOwned,
  ) -> Result<RawPassOutput<Self::OutputRead, Self::OutputOwned>, Self::Error> {
    cx.push(BuildDiagnostic::bundler_initialize_error(read.to_string(), None));
    Ok(token.finish((), ()))
  }
}

#[derive(Clone, Copy)]
struct FailingPass;

#[derive(Debug, PartialEq, Eq)]
struct TestError(&'static str);

impl Pass for FailingPass {
  type InputRead<'a> = ();
  type InputOwned = ();
  type OutputRead = ();
  type OutputOwned = ();
  type Error = TestError;

  fn run(
    self,
    _token: RunToken<'_, Self>,
    _cx: &mut PassCtx<'_>,
    (): Self::InputRead<'_>,
    (): Self::InputOwned,
  ) -> Result<RawPassOutput<Self::OutputRead, Self::OutputOwned>, Self::Error> {
    Err(TestError("typed failure"))
  }
}

#[test]
fn infallible_outputs_are_sealed_and_owned_outputs_continue() {
  let mut pipeline = PassPipelineCtx::new();

  let (length, owned) = run_infallible_pass(EmitPass, &mut pipeline, "alpha", 20);
  assert_eq!(*length, 5);
  assert_eq!(owned, 21);

  let (doubled, ()) = run_infallible_pass(ConsumeOwnedPass, &mut pipeline, (), owned);
  assert_eq!(*doubled, 42);
}

#[test]
fn fallible_pass_preserves_its_typed_error() {
  let mut pipeline = PassPipelineCtx::new();

  let result = run_pass(FailingPass, &mut pipeline, (), ());
  match result {
    Ok(_) => panic!("the failing pass unexpectedly succeeded"),
    Err(error) => assert_eq!(error, TestError("typed failure")),
  }
}

#[test]
fn serial_pipeline_preserves_diagnostic_order_and_provenance() {
  let recording = RecordingLayer::default();
  let subscriber = tracing_subscriber::registry().with(recording.clone());
  let messages = tracing::subscriber::with_default(subscriber, || {
    let mut pipeline = PassPipelineCtx::new();

    let _ = run_infallible_pass(EmitPass, &mut pipeline, "first", 0);
    let _ = run_infallible_pass(SecondEmitPass, &mut pipeline, "second", ());

    pipeline
      .into_diagnostics()
      .into_iter()
      .map(|diagnostic| diagnostic.to_string())
      .collect::<Vec<_>>()
  });

  assert_eq!(messages, ["first", "second"]);
  assert_eq!(
    recording.spans(),
    [
      SpanRecord {
        name: "run_pass",
        target: "rolldown::pass",
        pass: Some(type_name::<EmitPass>().to_string()),
      },
      SpanRecord {
        name: "run_pass",
        target: "rolldown::pass",
        pass: Some(type_name::<SecondEmitPass>().to_string()),
      },
    ]
  );
  assert_eq!(
    recording.events(),
    [
      EventRecord { target: "rolldown::pass", pass: Some(type_name::<EmitPass>().to_string()) },
      EventRecord {
        target: "rolldown::pass",
        pass: Some(type_name::<SecondEmitPass>().to_string()),
      },
    ]
  );
}
