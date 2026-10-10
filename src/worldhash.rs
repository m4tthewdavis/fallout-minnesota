//! `FMN_WORLD_HASH=1`: a fingerprint of the generated world, logged once at
//! start-up as a `world-hash:` line, so a refactor can prove it didn't move
//! a single crate, wolf or tree. Does nothing unless the variable is set.
//!
//! It runs in `PostStartup`, after every start-up spawner has been applied and
//! just before the generator is reseeded from the clock, so every part is the
//! same run to run. Each part is `count:hash` (FNV-1a 64 over the raw bytes),
//! so a mismatch says which part of the world changed.

use bevy::prelude::*;

use crate::enemy::{Body, Species};
use crate::interact::Container;
use crate::sim::collision::Shape;
use crate::sim::save::world_key;
use crate::state::{Colliders, RngRes, TreePositions};
use crate::weather_fx::Flake;
use crate::world::Pickup;

/// 64-bit FNV-1a: tiny, stable across builds and platforms, no dependencies.
#[derive(Clone, Copy)]
struct Fnv(u64);

impl Fnv {
    const OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
    const PRIME: u64 = 0x0000_0100_0000_01b3;

    fn new() -> Self {
        Fnv(Self::OFFSET)
    }
    fn bytes(&mut self, b: &[u8]) -> &mut Self {
        for &x in b {
            self.0 ^= x as u64;
            self.0 = self.0.wrapping_mul(Self::PRIME);
        }
        self
    }
    fn u64(&mut self, v: u64) -> &mut Self {
        self.bytes(&v.to_le_bytes())
    }
    fn i32(&mut self, v: i32) -> &mut Self {
        self.bytes(&v.to_le_bytes())
    }
    /// Exact bits, so even a last-place rounding change shows.
    fn f32(&mut self, v: f32) -> &mut Self {
        self.bytes(&v.to_bits().to_le_bytes())
    }
    fn str(&mut self, s: &str) -> &mut Self {
        // Length first, so "ab"+"c" and "a"+"bc" differ.
        self.u64(s.len() as u64).bytes(s.as_bytes())
    }
    fn finish(&self) -> u64 {
        self.0
    }
}

/// `count:hash` of items whose order means nothing (query results): each item
/// is hashed on its own, the hashes sorted, then hashed together.
fn unordered(mut items: Vec<u64>) -> String {
    items.sort_unstable();
    let mut h = Fnv::new();
    for i in &items {
        h.u64(*i);
    }
    format!("{}:{:016x}", items.len(), h.finish())
}

fn shape_hash(h: &mut Fnv, s: &Shape) {
    match *s {
        Shape::Circle { x, z, r } => h.bytes(&[0]).f32(x).f32(z).f32(r),
        Shape::Rect { x0, z0, x1, z1 } => h.bytes(&[1]).f32(x0).f32(z0).f32(x1).f32(z1),
        Shape::Capsule { x0, z0, x1, z1, r } => h.bytes(&[2]).f32(x0).f32(z0).f32(x1).f32(z1).f32(r),
    };
}

fn place_hash(h: &mut Fnv, p: Vec3) {
    let [kx, kz] = world_key(p.x, p.z);
    h.i32(kx).i32(kz).f32(p.x).f32(p.y).f32(p.z);
}

pub struct WorldHashPlugin;

impl Plugin for WorldHashPlugin {
    fn build(&self, app: &mut App) {
        if std::env::var("FMN_WORLD_HASH").is_err() {
            return;
        }
        app.add_systems(PostStartup, log_world_hash.before(crate::state::reseed_after_world));
    }
}

fn log_world_hash(
    rng: Res<RngRes>,
    colliders: Res<Colliders>,
    trees: Res<TreePositions>,
    pickups: Query<(&Transform, &Pickup)>,
    containers: Query<(&Transform, &Container)>,
    bodies: Query<(&Transform, &Body)>,
    flakes: Query<(&Transform, &Flake)>,
    everything: Query<()>,
) {
    let rng_hash = Fnv::new().str(&format!("{:?}", rng.0)).finish();

    // Collider order is kept: it's built in a fixed order and the grid reads it so.
    let mut h = Fnv::new();
    for s in &colliders.0 {
        shape_hash(&mut h, s);
    }
    let colliders = format!("{}:{:016x}", colliders.0.len(), h.finish());

    let mut h = Fnv::new();
    for &(x, z) in &trees.0 {
        h.f32(x).f32(z);
    }
    let trees = format!("{}:{:016x}", trees.0.len(), h.finish());

    let pickups = unordered(
        pickups
            .iter()
            .map(|(tf, p)| {
                let mut h = Fnv::new();
                place_hash(&mut h, tf.translation);
                h.str(&format!("{:?}", p.item)).f32(p.base_y).finish()
            })
            .collect(),
    );
    let containers = unordered(
        containers
            .iter()
            .map(|(tf, c)| {
                let mut h = Fnv::new();
                place_hash(&mut h, tf.translation);
                h.str(c.name).str(&format!("{:?}", c.loot)).finish()
            })
            .collect(),
    );
    // Creatures, one part per species, so a reordered spawner shows by name.
    let body = |want: fn(Species) -> bool| {
        unordered(
            bodies
                .iter()
                .filter(|(_, b)| want(b.species))
                .map(|(tf, b)| {
                    let mut h = Fnv::new();
                    place_hash(&mut h, tf.translation);
                    h.str(&format!("{:?}", b.species)).f32(tf.scale.x).finish()
                })
                .collect(),
        )
    };
    let wolves = body(|s| matches!(s, Species::Wolf { .. }));
    let moose = body(|s| s == Species::Moose);
    let crows = body(|s| s == Species::Crow);
    let raiders = body(|s| s == Species::Raider);
    let flakes = unordered(
        flakes
            .iter()
            .map(|(tf, f)| {
                let mut h = Fnv::new();
                h.u64(f.index as u64).f32(f.fall).f32(tf.translation.x).f32(tf.translation.y).f32(tf.translation.z);
                h.finish()
            })
            .collect(),
    );
    let entities = everything.iter().count();
    info!(
        "world-hash: rng={rng_hash:016x} colliders={colliders} trees={trees} pickups={pickups} containers={containers} \
         wolves={wolves} moose={moose} crows={crows} raiders={raiders} flakes={flakes} entities={entities}"
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fnv1a_matches_the_reference_vectors() {
        assert_eq!(Fnv::new().finish(), 0xcbf2_9ce4_8422_2325);
        assert_eq!(Fnv::new().bytes(b"a").finish(), 0xaf63_dc4c_8601_ec8c);
        assert_eq!(Fnv::new().bytes(b"foobar").finish(), 0x8594_4171_f739_67e8);
    }

    #[test]
    fn unordered_ignores_order_but_not_content() {
        assert_eq!(unordered(vec![1, 2, 3]), unordered(vec![3, 1, 2]));
        assert_ne!(unordered(vec![1, 2, 3]), unordered(vec![1, 2, 4]));
        assert!(unordered(vec![5, 6]).starts_with("2:"));
    }

    #[test]
    fn shapes_hash_their_kind_and_every_field() {
        let h = |s: Shape| {
            let mut f = Fnv::new();
            shape_hash(&mut f, &s);
            f.finish()
        };
        let c = h(Shape::Circle { x: 1.0, z: 2.0, r: 3.0 });
        assert_ne!(c, h(Shape::Circle { x: 1.0, z: 2.0, r: 3.000_001 }));
        assert_ne!(h(Shape::Rect { x0: 0.0, z0: 0.0, x1: 1.0, z1: 1.0 }), h(Shape::Capsule { x0: 0.0, z0: 0.0, x1: 1.0, z1: 1.0, r: 0.0 }));
    }
}
