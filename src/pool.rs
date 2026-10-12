//! How many threads each of Bevy's task pools gets.
//!
//! Bevy's default split gives the IO pool a quarter of the cores (one thread
//! on a 4-core machine), and every texture decode and glTF conversion runs on
//! it, so loading crawls on one core. Simply asking for more IO threads makes
//! Bevy take them out of the compute pool, which runs the game's systems, and
//! frames get slower. So the extra IO (and async compute) threads are added
//! *on top*: the compute pool keeps exactly the size Bevy would give it, and
//! the total thread count is raised to match. Once loading is over the IO
//! threads sleep, so a running game pays nothing for them.
//!
//! `FMN_POOL=bevy` restores Bevy's own split, for comparing load times.

use bevy::app::{TaskPoolOptions, TaskPoolThreadAssignmentPolicy};

/// Threads per pool.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PoolSplit {
    /// Asset loading: file reads, image decodes, glTF conversion.
    pub io: usize,
    /// Long background jobs: shader pipelines, mip chains.
    pub async_compute: usize,
    /// The game's systems and parallel queries.
    pub compute: usize,
}

impl PoolSplit {
    pub fn total(&self) -> usize {
        self.io + self.async_compute + self.compute
    }
}

/// Bevy 0.16's own rule (`TaskPoolOptions::default()` in bevy_app's
/// task_pool_plugin.rs): IO and async compute get 25% of the cores each
/// (rounded half up, at least 1, at most 4, never more than are left), and
/// compute gets whatever remains (at least 1).
pub fn bevy_default(cores: usize) -> PoolSplit {
    let total = cores.max(1);
    let share = |remaining: usize, percent: f32, min: usize, max: usize| -> usize {
        let proportion = total as f32 * percent;
        let mut desired = proportion as usize;
        if proportion - desired as f32 >= 0.5 {
            desired += 1;
        }
        desired.min(remaining).clamp(min, max)
    };
    let io = share(total, 0.25, 1, 4);
    let remaining = total.saturating_sub(io);
    let async_compute = share(remaining, 0.25, 1, 4);
    let remaining = remaining.saturating_sub(async_compute);
    let compute = share(remaining, 1.0, 1, usize::MAX);
    PoolSplit { io, async_compute, compute }
}

/// The game's split for `cores` logical cores: the compute pool exactly as
/// Bevy sizes it, IO raised to half the cores (2 to 6), async compute to at
/// least 2. Machines with fewer than 4 cores keep Bevy's split: they have no
/// spare core to load on.
pub fn split(cores: usize) -> PoolSplit {
    let base = bevy_default(cores);
    if cores < 4 {
        return base;
    }
    PoolSplit {
        io: base.io.max((cores / 2).clamp(2, 6)),
        async_compute: base.async_compute.max(2),
        compute: base.compute,
    }
}

/// Options for `TaskPoolPlugin` that give exactly `split`: every pool is pinned
/// to its count (min = max), and the total is pinned to their sum, so Bevy's
/// percentages can't move threads between pools.
pub fn options_for(split: PoolSplit) -> TaskPoolOptions {
    let d = TaskPoolOptions::default();
    let fixed = |n: usize, base: TaskPoolThreadAssignmentPolicy| TaskPoolThreadAssignmentPolicy { min_threads: n, max_threads: n, ..base };
    TaskPoolOptions {
        min_total_threads: split.total(),
        max_total_threads: split.total(),
        io: fixed(split.io, d.io),
        async_compute: fixed(split.async_compute, d.async_compute),
        compute: fixed(split.compute, d.compute),
    }
}

/// The task pools for this machine (or Bevy's defaults with `FMN_POOL=bevy`).
pub fn task_pool_options() -> TaskPoolOptions {
    if std::env::var("FMN_POOL").is_ok_and(|v| v == "bevy") {
        return TaskPoolOptions::default();
    }
    options_for(split(bevy::tasks::available_parallelism()))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Bevy's thread assignment (`TaskPoolOptions::create_default_pools` and
    /// `TaskPoolThreadAssignmentPolicy::get_number_of_threads`, bevy_app
    /// 0.16.1), replayed without creating any threads.
    fn assigned(o: &TaskPoolOptions, cores: usize) -> PoolSplit {
        let total = cores.clamp(o.min_total_threads, o.max_total_threads);
        let get = |p: &TaskPoolThreadAssignmentPolicy, remaining: usize| -> usize {
            assert!(p.percent >= 0.0);
            let proportion = total as f32 * p.percent;
            let mut desired = proportion as usize;
            if proportion - desired as f32 >= 0.5 {
                desired += 1;
            }
            desired.min(remaining).clamp(p.min_threads, p.max_threads)
        };
        let io = get(&o.io, total);
        let remaining = total.saturating_sub(io);
        let async_compute = get(&o.async_compute, remaining);
        let remaining = remaining.saturating_sub(async_compute);
        let compute = get(&o.compute, remaining);
        PoolSplit { io, async_compute, compute }
    }

    const CORES: [usize; 9] = [1, 2, 3, 4, 6, 8, 12, 16, 32];

    #[test]
    fn the_default_rule_is_bevys() {
        for cores in CORES {
            assert_eq!(bevy_default(cores), assigned(&TaskPoolOptions::default(), cores), "{cores} cores");
        }
    }

    #[test]
    fn the_split_at_each_core_count() {
        let s = |io, async_compute, compute| PoolSplit { io, async_compute, compute };
        assert_eq!(split(2), s(1, 1, 1));
        assert_eq!(split(4), s(2, 2, 2));
        assert_eq!(split(6), s(3, 2, 2));
        assert_eq!(split(8), s(4, 2, 4));
        assert_eq!(split(12), s(6, 3, 6));
        assert_eq!(split(16), s(6, 4, 8));
        assert_eq!(split(32), s(6, 4, 24));
    }

    #[test]
    fn compute_never_shrinks_and_extra_threads_are_added_on_top() {
        for cores in 1..=64 {
            let (d, s) = (bevy_default(cores), split(cores));
            assert_eq!(s.compute, d.compute, "{cores} cores");
            assert!(s.io >= d.io && s.async_compute >= d.async_compute, "{cores} cores");
            assert_eq!(s.total() - d.total(), (s.io - d.io) + (s.async_compute - d.async_compute));
            assert!(s.io <= 6 && s.async_compute <= 4);
        }
    }

    #[test]
    fn bevy_assigns_exactly_the_split_on_any_machine() {
        // The pinned options give the same pools whatever core count Bevy sees.
        for cores in CORES {
            let o = options_for(split(cores));
            for seen in [1, cores, cores * 2, 128] {
                assert_eq!(assigned(&o, seen), split(cores), "{cores} cores, Bevy sees {seen}");
            }
        }
    }
}
