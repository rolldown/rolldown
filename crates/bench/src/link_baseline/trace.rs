use std::{
  sync::{Arc, Mutex, PoisonError},
  thread::ThreadId,
  time::{Duration, Instant},
};

use rustc_hash::FxHashMap;
use serde::Serialize;
use tracing::{Level, Metadata, Subscriber, field::Visit, span::Attributes};
use tracing_subscriber::{Layer, layer::Context};

const LINK_SPAN_NAME: &str = "link";
const LINK_SPAN_TARGET: &str = "rolldown::stages::link_stage";
const PASS_SPAN_TARGET: &str = "rolldown::pass";

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TraceSpanTiming {
  pub name: String,
  pub target: String,
  pub pass: Option<String>,
  pub call_count: usize,
  pub inclusive_ns: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TraceSample {
  pub boundary_total_ns: u64,
  pub link_span_ns: u64,
  pub boundary_outside_link_span_ns: u64,
  pub direct_children_inclusive_sum_ns: u64,
  pub direct_children_wall_coverage_ns: u64,
  pub direct_children_overlap_excess_ns: u64,
  pub inside_link_unattributed_ns: u64,
  pub direct_children: Vec<TraceSpanTiming>,
  pub detached_passes: Vec<TraceSpanTiming>,
}

#[derive(Clone, Default)]
pub struct LinkTraceLayer {
  shared: Arc<Shared>,
}

#[derive(Default)]
struct Shared {
  state: Mutex<CollectorState>,
}

#[derive(Default)]
struct CollectorState {
  spans: Vec<SpanState>,
  live_ids: FxHashMap<u64, usize>,
  next_order: usize,
  errors: Vec<String>,
}

struct SpanState {
  name: &'static str,
  target: &'static str,
  pass: Option<String>,
  parent: Option<usize>,
  first_enter_order: Option<usize>,
  call_count: usize,
  active_entries: FxHashMap<ThreadId, Vec<Instant>>,
  intervals: Vec<Interval>,
}

#[derive(Clone, Copy)]
struct Interval {
  start: Instant,
  end: Instant,
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

impl LinkTraceLayer {
  /// Clears one completed sample while keeping the process-global subscriber installed.
  pub fn reset(&self) -> Result<(), String> {
    let mut state = self.shared.state.lock().unwrap_or_else(PoisonError::into_inner);
    if !state.live_ids.is_empty() || state.spans.iter().any(has_active_entries) {
      return Err(format!(
        "cannot reset the link trace collector while tracked spans are live: {}",
        live_span_names(&state).join(", ")
      ));
    }
    *state = CollectorState::default();
    Ok(())
  }

  pub fn sample(&self, boundary_total: Duration) -> Result<TraceSample, String> {
    let state = self.shared.state.lock().unwrap_or_else(PoisonError::into_inner);
    if !state.errors.is_empty() {
      return Err(format!("invalid span lifecycle: {}", state.errors.join("; ")));
    }
    if !state.live_ids.is_empty() || state.spans.iter().any(has_active_entries) {
      return Err(format!(
        "cannot sample the link trace collector while tracked spans are live: {}",
        live_span_names(&state).join(", ")
      ));
    }

    let links = state
      .spans
      .iter()
      .enumerate()
      .filter(|(_, span)| {
        span.name == LINK_SPAN_NAME
          && span.target == LINK_SPAN_TARGET
          && span.first_enter_order.is_some()
      })
      .collect::<Vec<_>>();
    let [(link_id, link)] = links.as_slice() else {
      return Err(format!(
        "the trace must record exactly one entered LinkStage::link span; recorded {}",
        links.len()
      ));
    };
    let link_id = *link_id;
    if link.call_count != 1 || link.intervals.len() != 1 {
      return Err(format!(
        "the LinkStage::link span must be entered exactly once; recorded {} enters and {} intervals",
        link.call_count,
        link.intervals.len()
      ));
    }

    let link_intervals = merge_intervals(link.intervals.iter().copied());
    let link_span = interval_sum(link.intervals.iter().copied());
    let boundary_outside_link_span = boundary_total.checked_sub(link_span).ok_or_else(|| {
      format!("the link span ({link_span:?}) exceeded the measured boundary ({boundary_total:?})")
    })?;

    let mut children = state
      .spans
      .iter()
      .filter(|span| span.parent == Some(link_id) && !span.intervals.is_empty())
      .collect::<Vec<_>>();
    children.sort_by_key(|span| span.first_enter_order);

    for child in &children {
      for interval in &child.intervals {
        if !interval_is_covered(*interval, &link_intervals) {
          return Err(format!(
            "direct child span `{}` has an interval outside LinkStage::link",
            child.name
          ));
        }
      }
    }

    let direct_children_inclusive_sum =
      interval_sum(children.iter().flat_map(|span| span.intervals.iter().copied()));
    let child_union =
      merge_intervals(children.iter().flat_map(|span| span.intervals.iter().copied()));
    let direct_children_wall_coverage = interval_sum(child_union.iter().copied());
    let direct_children_overlap_excess = direct_children_inclusive_sum
      .checked_sub(direct_children_wall_coverage)
      .ok_or_else(|| "direct-child interval union exceeded its inclusive sum".to_string())?;
    let inside_link_unattributed = link_span
      .checked_sub(direct_children_wall_coverage)
      .ok_or_else(|| "direct-child wall coverage exceeded the link span".to_string())?;

    let mut detached = state
      .spans
      .iter()
      .filter(|span| {
        span.target == PASS_SPAN_TARGET
          && span.parent != Some(link_id)
          && intervals_overlap(&span.intervals, &link_intervals)
      })
      .collect::<Vec<_>>();
    detached.sort_by_key(|span| span.first_enter_order);

    Ok(TraceSample {
      boundary_total_ns: duration_ns(boundary_total),
      link_span_ns: duration_ns(link_span),
      boundary_outside_link_span_ns: duration_ns(boundary_outside_link_span),
      direct_children_inclusive_sum_ns: duration_ns(direct_children_inclusive_sum),
      direct_children_wall_coverage_ns: duration_ns(direct_children_wall_coverage),
      direct_children_overlap_excess_ns: duration_ns(direct_children_overlap_excess),
      inside_link_unattributed_ns: duration_ns(inside_link_unattributed),
      direct_children: children.into_iter().map(span_timing).collect(),
      detached_passes: detached.into_iter().map(span_timing).collect(),
    })
  }
}

impl<S: Subscriber> Layer<S> for LinkTraceLayer {
  fn on_new_span(&self, attrs: &Attributes<'_>, id: &tracing::Id, ctx: Context<'_, S>) {
    let mut visitor = PassFieldVisitor::default();
    attrs.record(&mut visitor);
    let raw_parent = attrs.parent().map(tracing::Id::into_u64).or_else(|| {
      attrs.is_contextual().then(|| ctx.current_span().id().map(tracing::Id::into_u64)).flatten()
    });
    let metadata = attrs.metadata();
    let mut state = self.shared.state.lock().unwrap_or_else(PoisonError::into_inner);
    let parent = raw_parent.and_then(|parent| state.live_ids.get(&parent).copied());
    let is_link = metadata.name() == LINK_SPAN_NAME && metadata.target() == LINK_SPAN_TARGET;
    if !is_link && metadata.target() != PASS_SPAN_TARGET && parent.is_none() {
      return;
    }

    let stable_id = state.spans.len();
    let raw_id = id.into_u64();
    if state.live_ids.insert(raw_id, stable_id).is_some() {
      state.errors.push(format!("raw span id {raw_id} was reused before on_close"));
    }
    state.spans.push(SpanState {
      name: metadata.name(),
      target: metadata.target(),
      pass: visitor.pass,
      parent,
      first_enter_order: None,
      call_count: 0,
      active_entries: FxHashMap::default(),
      intervals: Vec::new(),
    });
  }

  fn on_enter(&self, id: &tracing::Id, _ctx: Context<'_, S>) {
    let mut state = self.shared.state.lock().unwrap_or_else(PoisonError::into_inner);
    let Some(stable_id) = state.live_ids.get(&id.into_u64()).copied() else {
      return;
    };
    let order = state.next_order;
    state.next_order += 1;
    let span = &mut state.spans[stable_id];
    span.call_count += 1;
    span.first_enter_order.get_or_insert(order);
    span.active_entries.entry(std::thread::current().id()).or_default().push(Instant::now());
  }

  fn on_exit(&self, id: &tracing::Id, _ctx: Context<'_, S>) {
    let ended = Instant::now();
    let mut state = self.shared.state.lock().unwrap_or_else(PoisonError::into_inner);
    let Some(stable_id) = state.live_ids.get(&id.into_u64()).copied() else {
      return;
    };
    let thread = std::thread::current().id();
    let span_name = state.spans[stable_id].name;
    let started = state.spans[stable_id].active_entries.get_mut(&thread).and_then(Vec::pop);
    if state.spans[stable_id].active_entries.get(&thread).is_some_and(Vec::is_empty) {
      state.spans[stable_id].active_entries.remove(&thread);
    }
    match started {
      Some(start) => state.spans[stable_id].intervals.push(Interval { start, end: ended }),
      None => state.errors.push(format!("span `{span_name}` exited without a matching enter")),
    }
  }

  fn on_close(&self, id: tracing::Id, _ctx: Context<'_, S>) {
    self
      .shared
      .state
      .lock()
      .unwrap_or_else(PoisonError::into_inner)
      .live_ids
      .remove(&id.into_u64());
  }
}

pub fn records_link_trace_metadata(metadata: &Metadata<'_>) -> bool {
  metadata.is_span() && *metadata.level() <= Level::DEBUG
}

fn span_timing(span: &SpanState) -> TraceSpanTiming {
  TraceSpanTiming {
    name: span.name.to_string(),
    target: span.target.to_string(),
    pass: span.pass.clone(),
    call_count: span.call_count,
    inclusive_ns: duration_ns(interval_sum(span.intervals.iter().copied())),
  }
}

fn live_span_names(state: &CollectorState) -> Vec<&'static str> {
  state
    .live_ids
    .values()
    .filter_map(|stable_id| state.spans.get(*stable_id).map(|span| span.name))
    .collect()
}

fn has_active_entries(span: &SpanState) -> bool {
  span.active_entries.values().any(|entries| !entries.is_empty())
}

fn merge_intervals(intervals: impl IntoIterator<Item = Interval>) -> Vec<Interval> {
  let mut intervals = intervals.into_iter().collect::<Vec<_>>();
  intervals.sort_by_key(|interval| interval.start);
  let mut merged: Vec<Interval> = Vec::with_capacity(intervals.len());
  for interval in intervals {
    if let Some(previous) = merged.last_mut()
      && interval.start <= previous.end
    {
      previous.end = previous.end.max(interval.end);
    } else {
      merged.push(interval);
    }
  }
  merged
}

fn interval_sum(intervals: impl IntoIterator<Item = Interval>) -> Duration {
  intervals.into_iter().fold(Duration::ZERO, |total, interval| {
    total.saturating_add(interval.end.saturating_duration_since(interval.start))
  })
}

fn interval_is_covered(interval: Interval, coverage: &[Interval]) -> bool {
  coverage.iter().any(|cover| cover.start <= interval.start && interval.end <= cover.end)
}

fn intervals_overlap(intervals: &[Interval], coverage: &[Interval]) -> bool {
  intervals.iter().any(|interval| {
    coverage.iter().any(|cover| interval.start <= cover.end && cover.start <= interval.end)
  })
}

fn duration_ns(duration: Duration) -> u64 {
  u64::try_from(duration.as_nanos()).unwrap_or(u64::MAX)
}
