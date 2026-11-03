use rolldown_error::{PluginTiming, PluginTimingKind, PluginTimingsMeasurement};

/// What one callback cost, measured inside it on the JavaScript side — the one place the
/// measurement exists, since this side can only bracket dispatch and completion.
#[napi_derive::napi(object, object_to_js = false)]
#[derive(Debug)]
pub struct BindingPluginTiming {
  /// The plugin the callback belongs to, or the options it was configured on.
  pub owner: String,
  #[napi(ts_type = "'plugin' | 'outputOption' | 'inputOption'")]
  pub kind: String,
  pub hook: String,
  pub calls: u32,
  pub ms: f64,
  pub max_in_flight: u32,
  /// How much of `ms` is double counted because calls overlapped.
  pub overlap_ms: f64,
  /// Whether `ms` may be compared against another row's — false once two calls of this hook
  /// overlapped, because then their spans cover work each other was doing.
  pub rankable: bool,
}

#[napi_derive::napi(object, object_to_js = false)]
#[derive(Debug)]
pub struct BindingPluginTimingsMeasurement {
  /// Wall time in which any measured callback was running, counting overlap once.
  pub busy_ms: f64,
  pub rows: Vec<BindingPluginTiming>,
}

impl From<BindingPluginTiming> for PluginTiming {
  fn from(row: BindingPluginTiming) -> Self {
    Self {
      owner: row.owner,
      // An unknown discriminant is not worth failing a build over, and every consumer of
      // `kind` today only groups by it.
      kind: match row.kind.as_str() {
        "outputOption" => PluginTimingKind::OutputOption,
        "inputOption" => PluginTimingKind::InputOption,
        _ => PluginTimingKind::Plugin,
      },
      hook: row.hook,
      calls: row.calls,
      ms: row.ms,
      max_in_flight: row.max_in_flight,
      overlap_ms: row.overlap_ms,
      rankable: row.rankable,
    }
  }
}

impl From<BindingPluginTimingsMeasurement> for PluginTimingsMeasurement {
  fn from(measurement: BindingPluginTimingsMeasurement) -> Self {
    Self {
      busy_ms: measurement.busy_ms,
      rows: measurement.rows.into_iter().map(Into::into).collect(),
    }
  }
}
