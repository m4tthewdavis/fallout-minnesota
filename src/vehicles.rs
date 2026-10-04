//! Abandoned vehicles: rusted 1950s sedans on the old highway (some on flat
//! tyres or bare rims, doors hanging open, glass smashed, one flipped onto
//! its roof), the tarp-covered car at the Bullseye-Mart, and snowmobiles
//! left out on the snow. Every one is set down with [`v::settle`] so its
//! wheels, skis or track touch the visible ground.

use std::f32::consts::FRAC_PI_2;

use bevy::pbr::NotShadowCaster;
use bevy::prelude::*;

use crate::assets::{tiled, GameAssets};
use crate::landmarks::{CARS, RUINS};
use crate::meshes::{to_mesh, to_mesh_tangents};
use crate::sim::collision::{self, Shape};
use crate::sim::meshgen;
use crate::sim::terrain;
use crate::sim::vehicles::{self as v, Window};
use crate::state::Colliders;
use crate::world::spawn_contact_shadow;

/// What's left of a wheel.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Wheel {
    Tyre,
    Flat,
    /// The tyre rotted away: a bare steel rim.
    Rim,
    /// Wheel gone, the corner resting on its brake drum.
    Missing,
}

impl Wheel {
    /// How high above the car's base the wheel's lowest point sits.
    fn lift(self) -> f32 {
        match self {
            Wheel::Tyre => 0.0,
            Wheel::Flat => v::WHEEL_R * 0.28,
            Wheel::Rim => v::WHEEL_R - v::RIM_R,
            Wheel::Missing => v::WHEEL_R - DRUM_R,
        }
    }
}

const DRUM_R: f32 = 0.14;

/// How each highway car was left, in the order of [`CARS`].
struct Wreck {
    paint: usize,
    /// Front-left, front-right, rear-left, rear-right.
    wheels: [Wheel; 4],
    /// Left and right front doors hanging open.
    doors: [bool; 2],
    broken: &'static [Window],
    /// Upside down on its roof, wheels in the air.
    flipped: bool,
    sink: f32,
}

use Wheel::*;
const WRECKS: [Wreck; 6] = [
    Wreck { paint: 0, wheels: [Tyre, Tyre, Tyre, Tyre], doors: [false, false], broken: &[], flipped: false, sink: 0.05 },
    Wreck { paint: 1, wheels: [Tyre, Flat, Tyre, Tyre], doors: [true, false], broken: &[Window::Windshield, Window::FrontLeft], flipped: false, sink: 0.06 },
    Wreck { paint: 2, wheels: [Rim, Rim, Rim, Rim], doors: [false, false], broken: &[Window::Rear, Window::RearLeft, Window::RearRight], flipped: false, sink: 0.08 },
    Wreck { paint: 3, wheels: [Tyre, Tyre, Flat, Tyre], doors: [false, false], broken: &[Window::Windshield, Window::FrontRight], flipped: true, sink: 0.12 },
    Wreck { paint: 1, wheels: [Tyre, Tyre, Tyre, Flat], doors: [false, true], broken: &[Window::FrontRight], flipped: false, sink: 0.05 },
    Wreck { paint: 2, wheels: [Rim, Tyre, Missing, Tyre], doors: [true, true], broken: &Window::ALL, flipped: false, sink: 0.06 },
];

/// The mesh and material handles shared by every car.
struct CarKit {
    body: Handle<Mesh>,
    underbody: Handle<Mesh>,
    fins: Handle<Mesh>,
    roof: Handle<Mesh>,
    pillars: Handle<Mesh>,
    chrome: Handle<Mesh>,
    trim: Handle<Mesh>,
    vinyl: Handle<Mesh>,
    dash: Handle<Mesh>,
    head_lens: Handle<Mesh>,
    tail_lens: Handle<Mesh>,
    door: Handle<Mesh>,
    openings: [Handle<Mesh>; 2],
    glass: Vec<(Window, Handle<Mesh>)>,
    tyre: Handle<Mesh>,
    flat: Handle<Mesh>,
    rim: Handle<Mesh>,
    hubcap: Handle<Mesh>,
    drum: Handle<Mesh>,
    plate: Handle<Mesh>,
    snow_cap: Handle<Mesh>,
    drift: Handle<Mesh>,
    paints: Vec<Handle<StandardMaterial>>,
    chrome_mat: Handle<StandardMaterial>,
    dark_mat: Handle<StandardMaterial>,
    glass_mat: Handle<StandardMaterial>,
    shard_mat: Handle<StandardMaterial>,
    vinyl_mat: Handle<StandardMaterial>,
    rubber_mat: Handle<StandardMaterial>,
    lens_mat: Handle<StandardMaterial>,
    tail_mat: Handle<StandardMaterial>,
    plate_mat: Handle<StandardMaterial>,
}

pub fn spawn_vehicles(
    mut commands: Commands,
    assets: Res<GameAssets>,
    server: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut colliders: ResMut<Colliders>,
) {
    let solid = &mut colliders.0;
    let kit = car_kit(&server, &mut meshes, &mut materials);
    for (i, &(x, z, yaw)) in CARS.iter().enumerate() {
        spawn_car(&mut commands, &mut meshes, &assets, &kit, &WRECKS[i % WRECKS.len()], i as u64, x, z, yaw);
        spawn_contact_shadow(&mut commands, &mut meshes, &assets, x, z, 1.3, 2.9, yaw);
        let (ax, az) = (yaw.sin() * 1.2, yaw.cos() * 1.2);
        solid.push(Shape::Circle { x: x + ax, z: z + az, r: 1.0 });
        solid.push(Shape::Circle { x: x - ax, z: z - az, r: 1.0 });
    }

    // The tarp-covered car in the Bullseye-Mart lot: its wheels are at
    // (+-0.72, +-1.29) in the model, bottoms at y = 0.
    let (bx, bz, _) = RUINS[0];
    let (px, pz, yaw) = (bx + 5.0, bz + 15.0, 0.4);
    let contacts = [(-0.72, 1.28, 0.0), (0.72, 1.28, 0.0), (-0.72, -1.3, 0.0), (0.72, -1.3, 0.0)];
    let rest = v::settle(px, pz, yaw, &contacts, 0.04, &terrain::mesh_height);
    commands.spawn((SceneRoot(assets.covered_car.clone()), rest_transform(px, pz, yaw, rest)));
    spawn_contact_shadow(&mut commands, &mut meshes, &assets, px, pz, 1.2, 2.6, yaw);
    for s in [-1.2f32, 1.2] {
        solid.push(Shape::Circle { x: px + yaw.sin() * s, z: pz + yaw.cos() * s, r: 1.0 });
    }

    spawn_snowmobiles(&mut commands, &mut meshes, &mut materials, &assets, solid);
}

fn rest_transform(x: f32, z: f32, yaw: f32, rest: v::Rest) -> Transform {
    Transform::from_xyz(x, rest.y, z).with_rotation(Quat::from_rotation_y(yaw) * Quat::from_rotation_x(rest.pitch) * Quat::from_rotation_z(rest.roll))
}

fn car_kit(server: &AssetServer, meshes: &mut Assets<Mesh>, materials: &mut Assets<StandardMaterial>) -> CarKit {
    let mut m = |d: meshgen::MeshData| meshes.add(to_mesh_tangents(&d));
    let (vinyl, dash) = v::sedan_interior();
    let (head, tail) = v::sedan_lenses();
    let glass = Window::ALL.iter().map(|&w| (w, m(v::window_glass(w)))).collect();
    let kit_meshes = (
        m(v::sedan_body()),
        m(v::sedan_underbody()),
        m(v::sedan_fins()),
        m(v::sedan_roof()),
        m(v::sedan_pillars()),
        m(v::sedan_chrome()),
        m(v::sedan_trim()),
        m(vinyl),
        m(dash),
        m(head),
        m(tail),
        m(v::sedan_door()),
        [m(v::door_opening(-1.0)), m(v::door_opening(1.0))],
    );
    let tyre = m(v::tyre(v::WHEEL_R, 0.22, 0.0));
    let flat = m(v::tyre(v::WHEEL_R, 0.22, 1.0));
    let rim = m(v::rim(v::RIM_R, 0.18));
    let hubcap = m(v::hubcap(0.2));
    let drum = m(meshgen::lathe(&[(0.0, -0.06), (DRUM_R, -0.06), (DRUM_R, 0.06), (0.0, 0.06)], 14, 0.5, false, false).rotated_z(-FRAC_PI_2));
    let plate = meshes.add(Rectangle::new(0.42, 0.21));
    let snow_cap = meshes.add(to_mesh_tangents(&meshgen::blob(1.0, 0.18, 0.18, 5, 1.5)));
    let drift = meshes.add(to_mesh_tangents(&meshgen::blob(1.0, 0.4, 0.22, 17, 1.5)));

    let paint_nor = tiled(server, "textures/generated/car_paint_nor.jpg".into(), false);
    let paint_arm = tiled(server, "textures/generated/car_paint_arm.jpg".into(), false);
    let paints = (0..4)
        .map(|k| {
            materials.add(StandardMaterial {
                base_color_texture: Some(tiled(server, format!("textures/generated/car_paint_{k}.jpg"), true)),
                normal_map_texture: Some(paint_nor.clone()),
                metallic_roughness_texture: Some(paint_arm.clone()),
                metallic: 1.0,
                perceptual_roughness: 1.0,
                ..default()
            })
        })
        .collect();
    let plain = |materials: &mut Assets<StandardMaterial>, c: Color, rough: f32, metal: f32| {
        materials.add(StandardMaterial {
            base_color: c,
            perceptual_roughness: rough,
            metallic: metal,
            ..default()
        })
    };
    let glass_mat = materials.add(StandardMaterial {
        // Frosted over by decades of winter.
        base_color: Color::srgba(0.78, 0.84, 0.88, 0.62),
        alpha_mode: AlphaMode::Blend,
        perceptual_roughness: 0.35,
        reflectance: 0.6,
        double_sided: true,
        cull_mode: None,
        ..default()
    });
    let shard_mat = materials.add(StandardMaterial {
        base_color: Color::srgba(0.7, 0.78, 0.82, 0.55),
        alpha_mode: AlphaMode::Blend,
        perceptual_roughness: 0.15,
        reflectance: 0.8,
        double_sided: true,
        cull_mode: None,
        ..default()
    });
    CarKit {
        body: kit_meshes.0,
        underbody: kit_meshes.1,
        fins: kit_meshes.2,
        roof: kit_meshes.3,
        pillars: kit_meshes.4,
        chrome: kit_meshes.5,
        trim: kit_meshes.6,
        vinyl: kit_meshes.7,
        dash: kit_meshes.8,
        head_lens: kit_meshes.9,
        tail_lens: kit_meshes.10,
        door: kit_meshes.11,
        openings: kit_meshes.12,
        glass,
        tyre,
        flat,
        rim,
        hubcap,
        drum,
        plate,
        snow_cap,
        drift,
        paints,
        // Pitted, grimy chrome.
        chrome_mat: plain(materials, Color::srgb(0.46, 0.43, 0.39), 0.42, 0.85),
        dark_mat: plain(materials, Color::srgb(0.05, 0.045, 0.04), 0.85, 0.2),
        glass_mat,
        shard_mat,
        vinyl_mat: plain(materials, Color::srgb(0.32, 0.12, 0.1), 0.75, 0.0),
        rubber_mat: plain(materials, Color::srgb(0.06, 0.06, 0.065), 0.93, 0.0),
        lens_mat: plain(materials, Color::srgb(0.82, 0.84, 0.8), 0.08, 0.1),
        tail_mat: materials.add(StandardMaterial {
            base_color: Color::srgb(0.6, 0.05, 0.04),
            emissive: LinearRgba::rgb(0.25, 0.0, 0.0),
            perceptual_roughness: 0.15,
            ..default()
        }),
        plate_mat: materials.add(StandardMaterial {
            base_color_texture: Some(tiled(server, "textures/generated/plate.png".into(), true)),
            perceptual_roughness: 0.6,
            metallic: 0.3,
            double_sided: true,
            cull_mode: None,
            ..default()
        }),
    }
}

fn spawn_car(commands: &mut Commands, meshes: &mut Assets<Mesh>, assets: &GameAssets, kit: &CarKit, wreck: &Wreck, seed: u64, x: f32, z: f32, yaw: f32) {
    let wheels = v::sedan_wheels();
    // A flipped car rests on its crumpled roof (and nose) instead of its wheels.
    let (contacts, inner) = if wreck.flipped {
        let c = vec![(-0.55, -0.7, 0.0), (0.55, -0.7, 0.0), (-0.55, 0.25, 0.0), (0.55, 0.25, 0.0)];
        let roof = 1.52;
        (c, Transform::from_xyz(0.0, roof, 0.0).with_rotation(Quat::from_rotation_z(std::f32::consts::PI) * Quat::from_rotation_x(-0.1)))
    } else {
        let c = wheels.iter().zip(wreck.wheels).map(|(&(wx, wz), w)| (wx, wz, w.lift())).collect();
        (c, Transform::IDENTITY)
    };
    let rest = v::settle(x, z, yaw, &contacts, wreck.sink, &terrain::mesh_height);
    let shards: Vec<Handle<Mesh>> = wreck.broken.iter().enumerate().map(|(k, &w)| meshes.add(to_mesh(&v::shards(&v::window_frame(w), seed * 7 + k as u64)))).collect();
    let paint = kit.paints[wreck.paint % kit.paints.len()].clone();
    let mesh = |h: &Handle<Mesh>, m: &Handle<StandardMaterial>| (Mesh3d(h.clone()), MeshMaterial3d(m.clone()));
    commands.spawn((rest_transform(x, z, yaw, rest), Visibility::default())).with_children(|root| {
        root.spawn((inner, Visibility::default())).with_children(|car| {
            for h in [&kit.body, &kit.fins, &kit.roof, &kit.pillars] {
                car.spawn(mesh(h, &paint));
            }
            car.spawn(mesh(&kit.underbody, &assets.rust));
            car.spawn(mesh(&kit.chrome, &kit.chrome_mat));
            car.spawn(mesh(&kit.trim, &kit.dark_mat));
            car.spawn(mesh(&kit.vinyl, &kit.vinyl_mat));
            car.spawn(mesh(&kit.dash, &kit.dark_mat));
            car.spawn((mesh(&kit.head_lens, &kit.lens_mat), NotShadowCaster));
            car.spawn((mesh(&kit.tail_lens, &kit.tail_mat), NotShadowCaster));
            car.spawn((mesh(&kit.plate, &kit.plate_mat), Transform::from_xyz(0.0, 0.67, -2.49).with_rotation(Quat::from_rotation_y(std::f32::consts::PI))));
            // Glass: intact panes, or jagged teeth where it was smashed. A
            // door left open took its window with it.
            for (w, h) in &kit.glass {
                let door_open = (*w == Window::FrontLeft && wreck.doors[0]) || (*w == Window::FrontRight && wreck.doors[1]);
                if door_open {
                    continue;
                }
                if let Some(k) = wreck.broken.iter().position(|b| b == w) {
                    car.spawn((mesh(&shards[k], &kit.shard_mat), NotShadowCaster));
                } else {
                    car.spawn((mesh(h, &kit.glass_mat), NotShadowCaster));
                }
            }
            // Open doors swing out on their front hinges.
            for (side_i, side) in [-1.0f32, 1.0].into_iter().enumerate() {
                if !wreck.doors[side_i] {
                    continue;
                }
                car.spawn(mesh(&kit.openings[side_i], &kit.dark_mat));
                let angle = 0.9 + 0.25 * ((seed + side_i as u64) % 3) as f32;
                car.spawn((
                    mesh(&kit.door, &paint),
                    Transform::from_xyz(side * (v::BODY_HALF_W - 0.02), 0.0, 0.85).with_rotation(Quat::from_rotation_y(-side * angle)),
                ));
            }
            // Wheels: tyre (full or flat) with rim and hubcap, a bare rim, or a drum.
            for (&(wx, wz), w) in wheels.iter().zip(wreck.wheels) {
                let side = wx.signum();
                let tf = Transform::from_xyz(wx, v::WHEEL_R, wz).with_rotation(Quat::from_rotation_y(if side < 0.0 { std::f32::consts::PI } else { 0.0 }));
                match w {
                    Tyre | Flat => {
                        car.spawn((mesh(if w == Flat { &kit.flat } else { &kit.tyre }, &kit.rubber_mat), tf));
                        car.spawn((mesh(&kit.rim, &assets.rust), tf));
                        if !(seed + wz.to_bits() as u64).is_multiple_of(3) {
                            car.spawn((mesh(&kit.hubcap, &kit.chrome_mat), tf.with_translation(tf.translation + Vec3::X * side * 0.07)));
                        }
                    }
                    Rim => {
                        car.spawn((mesh(&kit.rim, &assets.rust), tf));
                    }
                    Missing => {
                        car.spawn((mesh(&kit.drum, &assets.rust), tf));
                    }
                }
            }
            if !wreck.flipped {
                // Snow on the roof, hood and trunk.
                for (pos, s) in [
                    (Vec3::new(0.0, 1.53, -0.26), Vec3::new(0.6, 0.9, 0.52)),
                    (Vec3::new(0.0, 0.97, 1.62), Vec3::new(0.72, 0.7, 0.6)),
                    (Vec3::new(0.0, 0.98, -1.85), Vec3::new(0.72, 0.7, 0.45)),
                ] {
                    car.spawn((Mesh3d(kit.snow_cap.clone()), MeshMaterial3d(assets.snow.clone()), Transform::from_translation(pos).with_scale(s)));
                }
            }
        });
        // Snow drifted against the windward side and banked round the wheels.
        let windward = if seed.is_multiple_of(2) { -1.0 } else { 1.0 };
        root.spawn((
            Mesh3d(kit.drift.clone()),
            MeshMaterial3d(assets.snow.clone()),
            Transform::from_xyz(windward * 1.0, -0.05, -0.2).with_scale(Vec3::new(0.75, 1.0, 2.4)),
            NotShadowCaster,
        ));
        if wreck.flipped {
            // Snow settled on the upturned floor pan.
            root.spawn((Mesh3d(kit.snow_cap.clone()), MeshMaterial3d(assets.snow.clone()), Transform::from_xyz(0.0, 1.15, -0.1).with_scale(Vec3::new(0.75, 0.9, 1.9))));
        } else {
            for &(wx, wz) in &wheels {
                root.spawn((
                    Mesh3d(kit.drift.clone()),
                    MeshMaterial3d(assets.snow.clone()),
                    Transform::from_xyz(wx * 1.05, -0.04, wz).with_scale(Vec3::new(0.32, 0.5, 0.55)),
                    NotShadowCaster,
                ));
            }
        }
    });
}

fn spawn_snowmobiles(commands: &mut Commands, meshes: &mut Assets<Mesh>, materials: &mut Assets<StandardMaterial>, assets: &GameAssets, solid: &mut Vec<Shape>) {
    let mut m = |d: meshgen::MeshData| meshes.add(to_mesh_tangents(&d));
    let body = m(v::snowmobile_body());
    let black = m(v::snowmobile_black());
    let steel = m(v::snowmobile_steel());
    let track = m(v::snowmobile_track());
    let shield = m(v::snowmobile_windshield());
    let (head, tail) = v::snowmobile_lenses();
    let (head, tail) = (m(head), m(tail));
    let seat_snow = m(meshgen::blob(1.0, 0.25, 0.2, 77, 1.5));
    let drift = m(meshgen::blob(1.0, 0.4, 0.22, 31, 1.5));
    let plastic = |materials: &mut Assets<StandardMaterial>, c: Color| {
        materials.add(StandardMaterial {
            base_color: c,
            perceptual_roughness: 0.42,
            reflectance: 0.45,
            ..default()
        })
    };
    let black_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.05, 0.05, 0.055),
        perceptual_roughness: 0.7,
        ..default()
    });
    let steel_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.45, 0.45, 0.47),
        metallic: 0.85,
        perceptual_roughness: 0.45,
        ..default()
    });
    let rubber = materials.add(StandardMaterial {
        base_color: Color::srgb(0.07, 0.07, 0.075),
        perceptual_roughness: 0.95,
        ..default()
    });
    let smoke = materials.add(StandardMaterial {
        base_color: Color::srgba(0.25, 0.27, 0.3, 0.55),
        alpha_mode: AlphaMode::Blend,
        perceptual_roughness: 0.1,
        reflectance: 0.7,
        double_sided: true,
        cull_mode: None,
        ..default()
    });
    let lens = materials.add(StandardMaterial {
        base_color: Color::srgb(0.85, 0.85, 0.8),
        perceptual_roughness: 0.08,
        ..default()
    });
    let tail_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.7, 0.06, 0.04),
        emissive: LinearRgba::rgb(0.6, 0.02, 0.01),
        ..default()
    });
    let paints = [Color::srgb(0.88, 0.7, 0.12), Color::srgb(0.72, 0.16, 0.12), Color::srgb(0.15, 0.42, 0.62)];
    let spots = [(118.0f32, 68.0f32, 0.8f32), (-70.0, -28.0, -0.6), (165.0, 62.0, 2.2)];
    let contacts: Vec<(f32, f32, f32)> = v::snowmobile_contacts().iter().map(|&(x, z)| (x, z, 0.0)).collect();
    for (i, &(x, z, yaw)) in spots.iter().enumerate() {
        if terrain::lake_at(x, z).is_some() || collision::blocked(x, z, 1.0, solid) {
            continue;
        }
        let rest = v::settle(x, z, yaw, &contacts, 0.04 + 0.03 * i as f32, &terrain::mesh_height);
        let paint = plastic(materials, paints[i % paints.len()]);
        commands.spawn((rest_transform(x, z, yaw, rest), Visibility::default())).with_children(|s| {
            s.spawn((Mesh3d(body.clone()), MeshMaterial3d(paint)));
            s.spawn((Mesh3d(black.clone()), MeshMaterial3d(black_mat.clone())));
            s.spawn((Mesh3d(steel.clone()), MeshMaterial3d(steel_mat.clone())));
            s.spawn((Mesh3d(track.clone()), MeshMaterial3d(rubber.clone())));
            s.spawn((Mesh3d(shield.clone()), MeshMaterial3d(smoke.clone()), NotShadowCaster));
            s.spawn((Mesh3d(head.clone()), MeshMaterial3d(lens.clone()), NotShadowCaster));
            s.spawn((Mesh3d(tail.clone()), MeshMaterial3d(tail_mat.clone()), NotShadowCaster));
            // Snow on the seat, the cowl, and drifted round the track.
            s.spawn((Mesh3d(seat_snow.clone()), MeshMaterial3d(assets.snow.clone()), Transform::from_xyz(0.0, 0.85, -0.6).with_scale(Vec3::new(0.24, 0.7, 0.62))));
            s.spawn((Mesh3d(seat_snow.clone()), MeshMaterial3d(assets.snow.clone()), Transform::from_xyz(0.0, 0.84, 0.48).with_scale(Vec3::new(0.3, 0.5, 0.3))));
            s.spawn((
                Mesh3d(drift.clone()),
                MeshMaterial3d(assets.snow.clone()),
                Transform::from_xyz(if i % 2 == 0 { 0.62 } else { -0.62 }, -0.08, -0.8).with_scale(Vec3::new(0.32, 0.45, 1.0)),
                NotShadowCaster,
            ));
        });
        spawn_contact_shadow(commands, meshes, assets, x, z, 0.8, 1.9, yaw);
        let (ax, az) = (yaw.sin() * 0.6, yaw.cos() * 0.6);
        solid.push(Shape::Circle { x: x + ax, z: z + az, r: 0.7 });
        solid.push(Shape::Circle { x: x - ax, z: z - az, r: 0.7 });
    }
}
