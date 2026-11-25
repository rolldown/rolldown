use std::{
  collections::VecDeque,
  sync::{Arc, Mutex, MutexGuard, PoisonError},
};

use arcstr::ArcStr;
use rustc_hash::{FxHashMap, FxHashSet};

use super::client_have::ClientHave;

/// How many `Changed` updates the session keeps for catch-up. A client whose generation is
/// older than the oldest kept update reloads.
pub const HMR_CATCH_UP_RING_SIZE: usize = 64;

/// One `Changed` update, kept for catch-up. Ids only, no code.
#[derive(Debug, Clone)]
pub struct HmrRingEntry {
  pub generation: u32,
  pub changed_ids: Vec<ArcStr>,
  pub stale_ids: Vec<ArcStr>,
}

/// What a client must do before the server can answer its request.
#[derive(Debug)]
pub enum HmrCatchUp {
  UpToDate,
  /// The client missed the updates in `(client generation, generation]`. The payload tells
  /// it about them, oldest first.
  Behind {
    missed: Vec<HmrRingEntry>,
  },
  Reload {
    reason: String,
  },
}

/// Dev-engine-wide HMR state. It lives for the whole engine and survives full builds.
///
/// - `generation` goes up by 1 for each `Changed` update (`begin_rebuild`) and for each engine
///   full reload (`begin_full_reload`). `latest[m]` is the generation of the last `Changed`
///   update that stamped `m`. A module that never changed has no entry and stamp 0.
/// - Every module gets a number on first sight. The client sends sets of modules as sets of
///   these numbers (`HmrNumSet`). Numbers are never reused, so they stay valid across full
///   builds.
/// - `epoch` tells two engines apart: a client of another engine has numbers that mean other
///   modules.
///
/// Keys are `ArcStr` (the backing store of `StableModuleId`), so storing an id bumps a refcount.
#[derive(Debug, Default)]
pub struct HmrSession {
  epoch: u32,
  generation: u32,
  /// module stable id → the generation of the last `Changed` update that stamped it
  latest: FxHashMap<ArcStr, u32>,
  num_by_id: FxHashMap<ArcStr, u32>,
  id_by_num: Vec<ArcStr>,
  ring: VecDeque<HmrRingEntry>,
  /// The oldest client generation the session can still bring up to date.
  min_client_gen: u32,
}

impl HmrSession {
  pub fn new(epoch: u32) -> Self {
    Self { epoch, ..Self::default() }
  }

  pub fn epoch(&self) -> u32 {
    self.epoch
  }

  pub fn generation(&self) -> u32 {
    self.generation
  }

  /// Advances the generation and returns the new rebuild's stamp.
  pub fn begin_rebuild(&mut self) -> u32 {
    self.generation += 1;
    self.generation
  }

  /// Every client reloads. Modules can change without a stamp on this path (for example a
  /// tsconfig edit), so a client that loaded an older output must not be patched.
  pub fn begin_full_reload(&mut self) {
    self.generation += 1;
    self.ring.clear();
    self.min_client_gen = self.generation;
  }

  pub fn stamp(&mut self, module_stable_id: &ArcStr, generation: u32) {
    self.latest.insert(module_stable_id.clone(), generation);
  }

  /// The stamp a payload rendered right now records for `module_stable_id`.
  pub fn render_time_stamp(&self, module_stable_id: &str) -> u32 {
    self.latest.get(module_stable_id).copied().unwrap_or(0)
  }

  pub fn assign_num(&mut self, module_stable_id: &ArcStr) -> u32 {
    if let Some(num) = self.num_by_id.get(module_stable_id) {
      return *num;
    }
    let num = u32::try_from(self.id_by_num.len()).expect("fewer than 2^32 modules");
    self.num_by_id.insert(module_stable_id.clone(), num);
    self.id_by_num.push(module_stable_id.clone());
    num
  }

  pub fn num_of(&self, module_stable_id: &str) -> Option<u32> {
    self.num_by_id.get(module_stable_id).copied()
  }

  pub fn id_of(&self, num: u32) -> Option<&ArcStr> {
    self.id_by_num.get(num as usize)
  }

  /// How many numbers the session gave out. Every number is below it.
  pub fn num_count(&self) -> u32 {
    u32::try_from(self.id_by_num.len()).expect("fewer than 2^32 modules")
  }

  /// `generation` must be the value `begin_rebuild` just returned.
  pub fn record_update(&mut self, entry: HmrRingEntry) {
    debug_assert_eq!(entry.generation, self.generation);
    if self.ring.len() == HMR_CATCH_UP_RING_SIZE
      && let Some(evicted) = self.ring.pop_front()
    {
      self.min_client_gen = evicted.generation;
    }
    self.ring.push_back(entry);
  }

  /// Whether the client's copy of `module_stable_id` is the current one, given that its
  /// factories are fresh as of the client's generation.
  pub fn holds_current(&self, have: &ClientHave, module_stable_id: &str) -> bool {
    self.num_of(module_stable_id).is_some_and(|num| have.factories.contains(num))
      && self.render_time_stamp(module_stable_id) <= have.generation
  }

  /// The reason the session cannot serve this client, if any.
  pub fn reload_reason(&self, have: &ClientHave) -> Option<String> {
    if have.epoch != self.epoch {
      return Some("the client belongs to another dev engine".to_owned());
    }
    if have.generation > self.generation {
      return Some(format!("unknown generation {}", have.generation));
    }
    if have.generation < self.min_client_gen {
      return Some(format!("generation {} is too old to catch up", have.generation));
    }
    None
  }

  pub fn catch_up(&self, have: &ClientHave) -> HmrCatchUp {
    if let Some(reason) = self.reload_reason(have) {
      return HmrCatchUp::Reload { reason };
    }
    if have.generation == self.generation {
      return HmrCatchUp::UpToDate;
    }
    // Each id is listed only under the newest entry that names it. For a stale id, that entry's
    // generation is the id's stamp, so the client's floor for it ends up at the stamp.
    let mut seen_changed = FxHashSet::default();
    let mut seen_stale = FxHashSet::default();
    let mut missed = self
      .ring
      .iter()
      .rev()
      .take_while(|entry| entry.generation > have.generation)
      .map(|entry| HmrRingEntry {
        generation: entry.generation,
        changed_ids: entry
          .changed_ids
          .iter()
          .filter(|id| seen_changed.insert(*id))
          .cloned()
          .collect(),
        stale_ids: entry.stale_ids.iter().filter(|id| seen_stale.insert(*id)).cloned().collect(),
      })
      .filter(|entry| !entry.changed_ids.is_empty() || !entry.stale_ids.is_empty())
      .collect::<Vec<_>>();
    missed.reverse();
    HmrCatchUp::Behind { missed }
  }
}

/// The session shared by the bundler and every bundle it creates. Every lock is short and
/// taken in sync code only.
#[derive(Debug, Default, Clone)]
pub struct SharedHmrSession(Arc<Mutex<HmrSession>>);

impl SharedHmrSession {
  pub fn lock(&self) -> MutexGuard<'_, HmrSession> {
    self.0.lock().unwrap_or_else(PoisonError::into_inner)
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::hmr::client_have::HmrNumSet;

  fn have(epoch: u32, generation: u32) -> ClientHave {
    ClientHave { epoch, generation, ..ClientHave::default() }
  }

  #[test]
  fn numbers_are_append_only() {
    let mut session = HmrSession::new(7);
    let a = ArcStr::from("a.js");
    let b = ArcStr::from("b.js");
    assert_eq!(session.assign_num(&a), 0);
    assert_eq!(session.assign_num(&b), 1);
    assert_eq!(session.assign_num(&a), 0);
    assert_eq!(session.id_of(1).map(ArcStr::as_str), Some("b.js"));
    assert_eq!(session.num_of("c.js"), None);
  }

  #[test]
  fn catch_up_lists_each_id_under_its_newest_update() {
    let mut session = HmrSession::new(7);
    for (changed, stale) in [("a.js", "x.js"), ("b.js", "a.js")] {
      let generation = session.begin_rebuild();
      for id in [changed, stale] {
        session.stamp(&id.into(), generation);
      }
      session.record_update(HmrRingEntry {
        generation,
        changed_ids: vec![changed.into()],
        stale_ids: vec![changed.into(), stale.into()],
      });
    }
    assert!(matches!(session.catch_up(&have(7, 2)), HmrCatchUp::UpToDate));
    assert!(matches!(session.catch_up(&have(8, 2)), HmrCatchUp::Reload { .. }));
    assert!(matches!(session.catch_up(&have(7, 3)), HmrCatchUp::Reload { .. }));
    let HmrCatchUp::Behind { missed } = session.catch_up(&have(7, 0)) else {
      panic!("expected a catch-up");
    };
    let missed = missed
      .iter()
      .map(|entry| (entry.generation, entry.changed_ids.clone(), entry.stale_ids.clone()))
      .collect::<Vec<_>>();
    // `a.js` is stale in both updates, so it is listed only under the newer one.
    assert_eq!(
      missed,
      [
        (1, vec!["a.js".into()], vec!["x.js".into()]),
        (2, vec!["b.js".into()], vec!["b.js".into(), "a.js".into()]),
      ]
    );
    let HmrCatchUp::Behind { missed, .. } = session.catch_up(&have(7, 1)) else {
      panic!("expected a catch-up");
    };
    assert_eq!(missed.iter().map(|entry| entry.generation).collect::<Vec<_>>(), [2]);
  }

  #[test]
  fn catch_up_leaves_out_an_update_whose_ids_are_all_newer() {
    let mut session = HmrSession::new(7);
    for _ in 0..2 {
      let generation = session.begin_rebuild();
      session.stamp(&"a.js".into(), generation);
      session.record_update(HmrRingEntry {
        generation,
        changed_ids: vec!["a.js".into()],
        stale_ids: vec!["a.js".into()],
      });
    }
    let HmrCatchUp::Behind { missed, .. } = session.catch_up(&have(7, 0)) else {
      panic!("expected a catch-up");
    };
    assert_eq!(missed.iter().map(|entry| entry.generation).collect::<Vec<_>>(), [2]);
  }

  #[test]
  fn too_old_or_reloaded_clients_reload() {
    let mut session = HmrSession::new(1);
    for _ in 0..=HMR_CATCH_UP_RING_SIZE {
      let generation = session.begin_rebuild();
      session.record_update(HmrRingEntry {
        generation,
        changed_ids: vec!["a.js".into()],
        stale_ids: vec!["a.js".into()],
      });
    }
    assert!(matches!(session.catch_up(&have(1, 0)), HmrCatchUp::Reload { .. }));
    assert!(session.reload_reason(&have(1, 0)).is_some());
    assert!(matches!(session.catch_up(&have(1, 1)), HmrCatchUp::Behind { .. }));
    assert_eq!(session.reload_reason(&have(1, 1)), None);

    session.begin_full_reload();
    assert!(matches!(session.catch_up(&have(1, 10)), HmrCatchUp::Reload { .. }));
    assert!(matches!(session.catch_up(&have(1, session.generation())), HmrCatchUp::UpToDate));
  }

  #[test]
  fn holds_current_needs_the_factory_and_a_fresh_stamp() {
    let mut session = HmrSession::new(1);
    let a = ArcStr::from("a.js");
    let num = session.assign_num(&a);
    let generation = session.begin_rebuild();
    session.stamp(&a, generation);
    let mut client = have(1, generation);
    assert!(!session.holds_current(&client, "a.js"));
    client.factories = HmrNumSet::from_nums([num]);
    assert!(session.holds_current(&client, "a.js"));
    client.generation = generation - 1;
    assert!(!session.holds_current(&client, "a.js"));
  }
}
