//! The Pip-Boy: press Tab or M to pause the game and open a green CRT screen
//! with three tabs.
//!
//! * MAP: the whole 400 m area drawn from the real terrain (hills as contour
//!   lines, the nuclear ice, the highway, radiation zones, buildings, forest),
//!   fogged until you have explored it, with your position and heading, the
//!   places you have found, zoom (mouse wheel) and panning (drag, WASD).
//! * STATS: health, Body Heat, rads, weather, time, kills, exploration, audio.
//! * INVENTORY: weapons with ammo and upgrades, supplies, crafting.
//!
//! The drawing of the map and the fog rules are in `sim::mapdata`.

use bevy::input::mouse::{AccumulatedMouseMotion, AccumulatedMouseScroll};
use bevy::prelude::*;
use bevy::render::render_asset::RenderAssetUsages;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use bevy::image::ImageSampler;
use bevy::ui::widget::NodeImageMode;
use bevy::window::PrimaryWindow;

use crate::assets::GameAssets;
use crate::audio::{AudioSettings, GeigerOn};
use crate::player::{set_grab, Player};
use crate::sim::combat::{Upgrade, WeaponKind};
use crate::sim::mapdata::{self, Fog, Kind};
use crate::sim::survival::{Exposure, Inventory, Survival};
use crate::sim::synth::Sound;
use crate::sim::terrain::{self, HALF_SIZE};
use crate::sim::weather::Phase;
use crate::state::{ClockRes, Game, Messages, PipOpen, SfxQueue, TreePositions, WeatherRes};

const GREEN: Color = Color::srgb(0.45, 1.0, 0.45);
const DIM: Color = Color::srgba(0.45, 1.0, 0.45, 0.4);
/// Logical size of the square map view.
const VIEW: f32 = 560.0;
const MAP_PX: usize = 512;
const REVEAL_RADIUS: f32 = 42.0;
/// Which way UI rotation turns relative to a compass heading (checked on screen).
const ARROW_SIGN: f32 = -1.0;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Tab {
    Map,
    Stats,
    Inventory,
}

#[derive(Resource)]
struct PipState {
    tab: Tab,
    /// Map zoom (1 = whole map fits the view) and the world point at its centre.
    zoom: f32,
    center: Vec2,
    fog: Fog,
    base: Option<Handle<Image>>,
    fog_image: Handle<Image>,
    /// The fog picture needs re-uploading.
    dirty: bool,
}

#[derive(Component)]
struct PipRoot;
#[derive(Component)]
struct TabPage(Tab);
#[derive(Component)]
struct TabButton(Tab);
#[derive(Component)]
struct MapWorld;
#[derive(Component)]
struct MapBase;
#[derive(Component)]
struct PlayerMarker;
#[derive(Component)]
struct LandmarkMarker(usize);
#[derive(Component)]
struct LandmarkLabel(usize);
#[derive(Component)]
struct InfoText;
#[derive(Component)]
struct StatsText;
#[derive(Component)]
struct InventoryText;

pub struct PipboyPlugin;

impl Plugin for PipboyPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, build_ui)
            .add_systems(Update, auto_open.before(toggle_pipboy))
            .add_systems(Update, (update_fog, toggle_pipboy, (pipboy_input, update_map, update_pages, tab_buttons).chain().run_if(is_open)));
    }
}

/// Screenshot mode: FMN_PIP=map|stats|inventory opens the Pip-Boy after a moment
/// (FMN_PIP_ZOOM and FMN_PIP_REVEAL=1 tune the map, the latter lifting the fog).
fn auto_open(
    real: Res<Time<Real>>,
    mut stage: Local<u8>,
    mut keys: ResMut<ButtonInput<KeyCode>>,
    mut st: ResMut<PipState>,
    open: Res<PipOpen>,
) {
    let Ok(which) = std::env::var("FMN_PIP") else { return };
    let t = real.elapsed_secs();
    if *stage == 0 && t > 0.8 {
        *stage = 1;
        st.tab = match which.as_str() {
            "stats" => Tab::Stats,
            "inventory" => Tab::Inventory,
            _ => Tab::Map,
        };
        if let Some(z) = std::env::var("FMN_PIP_ZOOM").ok().and_then(|z| z.parse().ok()) {
            st.zoom = z;
        }
        if std::env::var("FMN_PIP_REVEAL").is_ok() {
            for x in (-200..200).step_by(20) {
                for z in (-200..200).step_by(20) {
                    st.fog.reveal(x as f32, z as f32, 30.0);
                }
            }
            for f in st.fog.landmarks_found.iter_mut() {
                *f = true;
            }
            st.dirty = true;
        }
    }
    if *stage == 1 && t > 1.2 && !open.0 {
        *stage = 2;
        keys.press(KeyCode::Tab);
    }
}

/// Keep the map filling the view: the centre can't go so far that an edge of
/// the map shows black.
fn clamp_center(c: Vec2, zoom: f32) -> Vec2 {
    let reach = (HALF_SIZE - HALF_SIZE / zoom).max(0.0);
    c.clamp(Vec2::splat(-reach), Vec2::splat(reach))
}

fn is_open(open: Res<PipOpen>) -> bool {
    open.0
}

fn rgba_image(w: u32, h: u32, data: Vec<u8>) -> Image {
    let mut img = Image::new(
        Extent3d {
            width: w,
            height: h,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    );
    img.sampler = ImageSampler::linear();
    img
}

fn build_ui(mut commands: Commands, assets: Res<GameAssets>, mut images: ResMut<Assets<Image>>) {
    let fog = Fog::new();
    let fog_image = images.add(rgba_image(mapdata::FOG_CELLS as u32, mapdata::FOG_CELLS as u32, fog.overlay()));
    let arrow = images.add(rgba_image(32, 32, mapdata::arrow_icon(32)));
    commands.insert_resource(PipState {
        tab: Tab::Map,
        zoom: 1.0,
        center: Vec2::ZERO,
        fog,
        base: None,
        fog_image: fog_image.clone(),
        dirty: false,
    });

    let font = |size: f32| TextFont {
        font: assets.font.clone(),
        font_size: size,
        ..default()
    };

    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                row_gap: Val::Px(10.0),
                ..default()
            },
            BackgroundColor(Color::srgba(0.01, 0.045, 0.02, 0.97)),
            GlobalZIndex(100),
            Visibility::Hidden,
            PipRoot,
        ))
        .with_children(|root| {
            // Title and tabs.
            root.spawn(Node {
                width: Val::Px(900.0),
                justify_content: JustifyContent::SpaceBetween,
                align_items: AlignItems::Center,
                ..default()
            })
            .with_children(|bar| {
                bar.spawn((Text::new("PIP-BOY 3000"), font(24.0), TextColor(GREEN)));
                bar.spawn(Node {
                    column_gap: Val::Px(8.0),
                    ..default()
                })
                .with_children(|tabs| {
                    for (tab, label) in [(Tab::Map, "1 MAP"), (Tab::Stats, "2 STATS"), (Tab::Inventory, "3 INVENTORY")] {
                        tabs.spawn((
                            Button,
                            Node {
                                padding: UiRect::axes(Val::Px(14.0), Val::Px(5.0)),
                                border: UiRect::all(Val::Px(1.0)),
                                ..default()
                            },
                            BorderColor(DIM),
                            BackgroundColor(Color::NONE),
                            TabButton(tab),
                        ))
                        .with_children(|b| {
                            b.spawn((Text::new(label), font(17.0), TextColor(GREEN)));
                        });
                    }
                });
            });

            // Page area.
            root.spawn(Node {
                width: Val::Px(900.0),
                height: Val::Px(VIEW + 8.0),
                border: UiRect::all(Val::Px(1.0)),
                ..default()
            })
            .insert(BorderColor(DIM))
            .with_children(|area| {
                // ---- MAP ----
                area.spawn((
                    Node {
                        width: Val::Percent(100.0),
                        height: Val::Percent(100.0),
                        column_gap: Val::Px(14.0),
                        padding: UiRect::all(Val::Px(3.0)),
                        ..default()
                    },
                    TabPage(Tab::Map),
                ))
                .with_children(|page| {
                    page.spawn((
                        Node {
                            width: Val::Px(VIEW),
                            height: Val::Px(VIEW),
                            overflow: Overflow::clip(),
                            border: UiRect::all(Val::Px(1.0)),
                            ..default()
                        },
                        BorderColor(DIM),
                        BackgroundColor(Color::srgb(0.01, 0.04, 0.02)),
                    ))
                    .with_children(|view| {
                        view.spawn((
                            Node {
                                position_type: PositionType::Absolute,
                                width: Val::Px(VIEW),
                                height: Val::Px(VIEW),
                                ..default()
                            },
                            MapWorld,
                        ))
                        .with_children(|world| {
                            let stretch = |h: &Handle<Image>| ImageNode {
                                image: h.clone(),
                                image_mode: NodeImageMode::Stretch,
                                ..default()
                            };
                            let fill = Node {
                                position_type: PositionType::Absolute,
                                width: Val::Percent(100.0),
                                height: Val::Percent(100.0),
                                ..default()
                            };
                            world.spawn((fill.clone(), ImageNode::default(), MapBase));
                            world.spawn((fill, stretch(&fog_image)));
                            // Landmarks: a coloured diamond and its name, shown once found.
                            for (i, lm) in mapdata::landmarks().iter().enumerate() {
                                let (u, v) = mapdata::to_uv(lm.x, lm.z);
                                let colour = match lm.kind {
                                    Kind::Vault => Color::srgb(1.0, 0.9, 0.3),
                                    Kind::Shelter => Color::srgb(1.0, 0.65, 0.25),
                                    Kind::Ruin => Color::srgb(0.9, 1.0, 0.8),
                                    Kind::Hazard => Color::srgb(1.0, 0.35, 0.25),
                                    Kind::Camp => Color::srgb(0.6, 0.85, 1.0),
                                    Kind::Water => Color::srgb(0.4, 1.0, 0.85),
                                };
                                world.spawn((
                                    Node {
                                        position_type: PositionType::Absolute,
                                        left: Val::Percent(u * 100.0),
                                        top: Val::Percent(v * 100.0),
                                        width: Val::Px(9.0),
                                        height: Val::Px(9.0),
                                        margin: UiRect::all(Val::Px(-4.5)),
                                        ..default()
                                    },
                                    BackgroundColor(colour),
                                    Transform::from_rotation(Quat::from_rotation_z(0.785)),
                                    Visibility::Hidden,
                                    LandmarkMarker(i),
                                ));
                                world
                                    .spawn((
                                        Node {
                                            position_type: PositionType::Absolute,
                                            left: Val::Percent(u * 100.0),
                                            top: Val::Percent(v * 100.0),
                                            margin: UiRect {
                                                left: Val::Px(9.0),
                                                top: Val::Px(-8.0),
                                                ..default()
                                            },
                                            ..default()
                                        },
                                        Visibility::Hidden,
                                        LandmarkLabel(i),
                                    ))
                                    .with_children(|label| {
                                        label.spawn((Text::new(lm.name), font(13.0), TextColor(colour)));
                                    });
                            }
                            // You.
                            world.spawn((
                                Node {
                                    position_type: PositionType::Absolute,
                                    left: Val::Percent(50.0),
                                    top: Val::Percent(50.0),
                                    width: Val::Px(24.0),
                                    height: Val::Px(24.0),
                                    margin: UiRect::all(Val::Px(-12.0)),
                                    ..default()
                                },
                                ImageNode {
                                    image: arrow.clone(),
                                    image_mode: NodeImageMode::Stretch,
                                    ..default()
                                },
                                PlayerMarker,
                            ));
                        });
                    });
                    // Info column.
                    page.spawn(Node {
                        flex_direction: FlexDirection::Column,
                        width: Val::Px(300.0),
                        row_gap: Val::Px(6.0),
                        ..default()
                    })
                    .with_children(|col| {
                        col.spawn((Text::new(""), font(15.0), TextColor(GREEN), InfoText));
                    });
                });
                // ---- STATS ----
                area.spawn((
                    Node {
                        width: Val::Percent(100.0),
                        height: Val::Percent(100.0),
                        padding: UiRect::all(Val::Px(18.0)),
                        display: Display::None,
                        ..default()
                    },
                    TabPage(Tab::Stats),
                ))
                .with_children(|p| {
                    p.spawn((Text::new(""), font(18.0), TextColor(GREEN), StatsText));
                });
                // ---- INVENTORY ----
                area.spawn((
                    Node {
                        width: Val::Percent(100.0),
                        height: Val::Percent(100.0),
                        padding: UiRect::all(Val::Px(18.0)),
                        display: Display::None,
                        ..default()
                    },
                    TabPage(Tab::Inventory),
                ))
                .with_children(|p| {
                    p.spawn((Text::new(""), font(17.0), TextColor(GREEN), InventoryText));
                });
            });
            root.spawn((
                Text::new("TAB / M close    1 2 3 tabs    MAP: wheel zoom, drag or WASD pan, C centre on you"),
                font(14.0),
                TextColor(DIM),
            ));
        });
}

/// Reveal the map around you, find landmarks, and refresh the fog picture.
fn update_fog(
    mut st: ResMut<PipState>,
    mut images: ResMut<Assets<Image>>,
    mut msgs: ResMut<Messages>,
    mut sfx: ResMut<SfxQueue>,
    player: Query<&Transform, With<Player>>,
) {
    let Ok(p) = player.single() else { return };
    let (x, z) = (p.translation.x, p.translation.z);
    if st.fog.reveal(x, z, REVEAL_RADIUS) || std::mem::take(&mut st.dirty) {
        if let Some(img) = images.get_mut(&st.fog_image) {
            img.data = Some(st.fog.overlay());
        }
    }
    let marks = mapdata::landmarks();
    for i in st.fog.discover(x, z) {
        // The vault is where you start; announcing it would be noise.
        if marks[i].kind != Kind::Vault {
            msgs.show(format!("Discovered: {}", marks[i].name), 3.5);
            sfx.play_gain(Sound::Craft, 0.7);
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn toggle_pipboy(
    keys: Res<ButtonInput<KeyCode>>,
    mut open: ResMut<PipOpen>,
    game: Res<Game>,
    mut st: ResMut<PipState>,
    trees: Res<TreePositions>,
    mut images: ResMut<Assets<Image>>,
    mut vtime: ResMut<Time<Virtual>>,
    mut sfx: ResMut<SfxQueue>,
    mut windows: Query<&mut Window, With<PrimaryWindow>>,
    mut root: Query<&mut Visibility, With<PipRoot>>,
    mut base: Query<&mut ImageNode, With<MapBase>>,
    player: Query<&Transform, With<Player>>,
) {
    let want_toggle = keys.just_pressed(KeyCode::Tab) || keys.just_pressed(KeyCode::KeyM);
    let want_close = open.0 && keys.just_pressed(KeyCode::Escape);
    if !(want_toggle || want_close) {
        return;
    }
    if !open.0 && game.death.is_some() {
        return;
    }
    open.0 = !open.0;
    info!("pip-boy {}", if open.0 { "opened" } else { "closed" });
    let Ok(mut vis) = root.single_mut() else { return };
    if open.0 {
        // Draw the map the first time it is needed.
        if st.base.is_none() {
            let started = std::time::Instant::now();
            let data = mapdata::render_base(MAP_PX, &trees.0);
            info!("pip-boy map drawn in {:.2}s", started.elapsed().as_secs_f32());
            let handle = images.add(rgba_image(MAP_PX as u32, MAP_PX as u32, data));
            st.base = Some(handle);
        }
        if let (Some(handle), Ok(mut node)) = (st.base.clone(), base.single_mut()) {
            node.image = handle;
            node.image_mode = NodeImageMode::Stretch;
        }
        if let Ok(p) = player.single() {
            let (zoom, at) = (st.zoom, Vec2::new(p.translation.x, p.translation.z));
            st.center = clamp_center(at, zoom);
        }
        *vis = Visibility::Visible;
        vtime.pause();
        if let Ok(mut w) = windows.single_mut() {
            set_grab(&mut w, false);
        }
        sfx.play(Sound::PipOn);
    } else {
        *vis = Visibility::Hidden;
        vtime.unpause();
        if let Ok(mut w) = windows.single_mut() {
            set_grab(&mut w, true);
        }
        sfx.play(Sound::PipOff);
    }
}

#[allow(clippy::too_many_arguments)]
fn pipboy_input(
    time: Res<Time<Real>>,
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    motion: Res<AccumulatedMouseMotion>,
    scroll: Res<AccumulatedMouseScroll>,
    mut st: ResMut<PipState>,
    mut sfx: ResMut<SfxQueue>,
    player: Query<&Transform, With<Player>>,
) {
    for (key, tab) in [(KeyCode::Digit1, Tab::Map), (KeyCode::Digit2, Tab::Stats), (KeyCode::Digit3, Tab::Inventory)] {
        if keys.just_pressed(key) && st.tab != tab {
            st.tab = tab;
            sfx.play(Sound::UiTab);
        }
    }
    if st.tab != Tab::Map {
        return;
    }
    let dt = time.delta_secs();
    // Zoom with the wheel or +/-.
    let mut zoom = st.zoom;
    zoom *= 1.0 + scroll.delta.y * 0.12;
    if keys.pressed(KeyCode::Equal) || keys.pressed(KeyCode::NumpadAdd) {
        zoom *= 1.0 + dt * 1.2;
    }
    if keys.pressed(KeyCode::Minus) || keys.pressed(KeyCode::NumpadSubtract) {
        zoom /= 1.0 + dt * 1.2;
    }
    st.zoom = zoom.clamp(1.0, 6.0);
    // Pan: drag with the mouse, or WASD / arrows. One logical pixel of drag is
    // WORLD / (VIEW * zoom) metres.
    let metres_per_px = mapdata::WORLD / (VIEW * st.zoom);
    let mut c = st.center;
    if mouse.pressed(MouseButton::Left) {
        c -= Vec2::new(motion.delta.x, motion.delta.y) * metres_per_px;
    }
    let pan = 140.0 / st.zoom * dt;
    for (key, d) in [
        (KeyCode::KeyW, Vec2::NEG_Y),
        (KeyCode::ArrowUp, Vec2::NEG_Y),
        (KeyCode::KeyS, Vec2::Y),
        (KeyCode::ArrowDown, Vec2::Y),
        (KeyCode::KeyA, Vec2::NEG_X),
        (KeyCode::ArrowLeft, Vec2::NEG_X),
        (KeyCode::KeyD, Vec2::X),
        (KeyCode::ArrowRight, Vec2::X),
    ] {
        if keys.pressed(key) {
            c += d * pan;
        }
    }
    if keys.just_pressed(KeyCode::KeyC) {
        if let Ok(p) = player.single() {
            c = Vec2::new(p.translation.x, p.translation.z);
        }
    }
    st.center = clamp_center(c, st.zoom);
}

/// Place the map, markers and player arrow for the current zoom and centre.
#[allow(clippy::type_complexity)]
fn update_map(
    st: Res<PipState>,
    player: Query<(&Transform, &Player)>,
    mut world: Query<&mut Node, (With<MapWorld>, Without<PlayerMarker>)>,
    mut markers: Query<(&mut Visibility, &LandmarkMarker), Without<LandmarkLabel>>,
    mut labels: Query<(&mut Visibility, &LandmarkLabel), Without<LandmarkMarker>>,
    mut arrow: Query<(&mut Transform, &mut Node), (With<PlayerMarker>, Without<MapWorld>, Without<Player>)>,
    mut info: Query<&mut Text, With<InfoText>>,
    game: Res<Game>,
    clock: Res<ClockRes>,
) {
    let size = VIEW * st.zoom;
    if let Ok(mut n) = world.single_mut() {
        let (u, v) = mapdata::to_uv(st.center.x, st.center.y);
        n.width = Val::Px(size);
        n.height = Val::Px(size);
        n.left = Val::Px(VIEW * 0.5 - u * size);
        n.top = Val::Px(VIEW * 0.5 - v * size);
    }
    let found = &st.fog.landmarks_found;
    for (mut vis, m) in &mut markers {
        *vis = if found[m.0] { Visibility::Inherited } else { Visibility::Hidden };
    }
    for (mut vis, l) in &mut labels {
        // Names clutter the whole-map view, so show them as you zoom in (and always for the vault).
        let show = found[l.0] && (st.zoom >= 1.6 || mapdata::landmarks()[l.0].kind == Kind::Vault);
        *vis = if show { Visibility::Inherited } else { Visibility::Hidden };
    }
    let Ok((ptf, p)) = player.single() else { return };
    let (u, v) = mapdata::to_uv(ptf.translation.x, ptf.translation.z);
    if let Ok((mut tf, mut n)) = arrow.single_mut() {
        n.left = Val::Percent(u * 100.0);
        n.top = Val::Percent(v * 100.0);
        // The arrow points up at rotation 0; heading is clockwise from north.
        tf.rotation = Quat::from_rotation_z(-p.yaw * ARROW_SIGN);
    }
    if let Ok(mut t) = info.single_mut() {
        let marks = mapdata::landmarks();
        let nearest = marks
            .iter()
            .enumerate()
            .filter(|(i, _)| found[*i])
            .map(|(_, l)| (l, (l.x - ptf.translation.x).hypot(l.z - ptf.translation.z)))
            .min_by(|a, b| a.1.total_cmp(&b.1));
        let (cx, cz) = (ptf.translation.x, ptf.translation.z);
        let heading = (-p.yaw).to_degrees().rem_euclid(360.0);
        let places = found.iter().filter(|f| **f).count();
        let mut s = format!(
            "YOU ARE HERE\n  X {:>4.0}   Z {:>4.0}\n  Heading {:03.0}\n  Surface: {:?}\n\n",
            cx,
            cz,
            heading,
            terrain::surface_at(cx, cz)
        );
        if let Some((l, d)) = nearest {
            s += &format!("NEAREST PLACE\n  {}\n  {:.0} m away\n\n", l.name, d);
        }
        s += &format!(
            "EXPLORED  {:.0}%\nPLACES FOUND  {}/{}\nDAY {}  {}\n\n",
            st.fog.fraction() * 100.0,
            places,
            marks.len(),
            clock.0.day,
            clock.0.label()
        );
        s += "LEGEND\n  yellow  Vault 143\n  orange  fish house (warmth,\n          workbench)\n  white   ruins (loot!)\n  red     hazard\n  blue    old camp\n  green   nuclear ice, radiation\n";
        if game.weapon().kind != WeaponKind::IceAxe && game.inv.scrap > 0 {
            s += &format!("\nYou carry {} scrap.", game.inv.scrap);
        }
        t.0 = s;
    }
}

/// Show the page for the selected tab and refresh its text.
#[allow(clippy::too_many_arguments)]
fn update_pages(
    st: Res<PipState>,
    game: Res<Game>,
    weather: Res<WeatherRes>,
    clock: Res<ClockRes>,
    audio: Res<AudioSettings>,
    geiger: Res<GeigerOn>,
    player: Query<&Transform, With<Player>>,
    mut pages: Query<(&mut Node, &TabPage)>,
    mut stats: Query<&mut Text, (With<StatsText>, Without<InventoryText>, Without<InfoText>)>,
    mut inventory: Query<&mut Text, (With<InventoryText>, Without<StatsText>, Without<InfoText>)>,
) {
    for (mut node, page) in &mut pages {
        let want = if page.0 == st.tab { Display::Flex } else { Display::None };
        if node.display != want {
            node.display = want;
        }
    }
    let s = &game.survival;
    let pos = player.single().map(|t| t.translation).unwrap_or(Vec3::ZERO);
    let sheltered = terrain::shelter_at(pos.x, pos.z).is_some();
    let cond = weather.weather.conditions();
    let air = cond.air_temp_f + clock.0.temp_offset_f();
    let feels = Survival::effective_temp(&Exposure {
        air_temp_f: air,
        wind_chill_f: cond.wind_chill_f,
        sheltered,
        ..Default::default()
    });
    let phase = match weather.weather.phase {
        Phase::Calm => "calm",
        Phase::Warning => "SIREN: blizzard coming",
        Phase::Blizzard => "RAD-BLIZZARD",
    };
    let pct = |v: f32| (v * 100.0).round() as i32;
    if let Ok(mut t) = stats.single_mut() {
        t.0 = format!(
            "THAWBORN OF VAULT 143                      DAY {}  {}\n\n\
             HEALTH      {:>3.0} / {:.0}   (radiation has taken {:.0} off your maximum)\n\
             BODY HEAT   {:>3.0}%{}\n\
             RADIATION   {:>4.0} / {:.0}\n\n\
             AIR  {:.0}F   feels like {:.0}F   weather: {}\n\n\
             KILLS {}      EXPLORED {:.0}%      PLACES FOUND {}/{}\n\n\
             AUDIO     master {}%   effects {}%   music {}%   ambience {}%{}\n\
             GEIGER COUNTER {}\n\
             (F10/F11 master, F5/F6 effects, F7/F8 music, F9 mute, G Geiger)",
            clock.0.day,
            clock.0.label(),
            s.health,
            s.max_health(),
            Survival::BASE_MAX_HEALTH - s.max_health(),
            s.body_heat,
            if s.frostbite { "   FROSTBITE" } else { "" },
            s.rads,
            Survival::MAX_RADS,
            air,
            feels,
            phase,
            game.kills,
            st.fog.fraction() * 100.0,
            st.fog.landmarks_found.iter().filter(|f| **f).count(),
            st.fog.landmarks_found.len(),
            pct(audio.0.master),
            pct(audio.0.sfx),
            pct(audio.0.music),
            pct(audio.0.ambience),
            if audio.0.muted { "   MUTED" } else { "" },
            if geiger.0 { "ON" } else { "OFF" },
        );
    }
    if let Ok(mut t) = inventory.single_mut() {
        let inv = &game.inv;
        let mut out = String::from("WEAPONS\n");
        for kind in WeaponKind::ALL {
            let i = kind.slot();
            if !game.arsenal.owned[i] {
                out += &format!("  [{}] ---  not found yet\n", i + 1);
                continue;
            }
            let w = game.arsenal.get(kind);
            let ammo = match kind.ammo() {
                Some(a) => format!("{}/{}  (+{} {})", w.mag, w.mag_size, inv.reserve(a), a.name()),
                None => "melee".to_string(),
            };
            let fitted: Vec<&str> = Upgrade::ALL.iter().filter(|u| w.has_upgrade(**u)).map(|u| u.name()).collect();
            out += &format!(
                "{} [{}] {:<18} {:<34} {}\n",
                if game.arsenal.current == i { ">" } else { " " },
                i + 1,
                kind.name(),
                ammo,
                if fitted.is_empty() { String::new() } else { format!("fitted: {}", fitted.join(", ")) }
            );
        }
        out += &format!(
            "\nSUPPLIES\n  Stimpak x{}   RadAway x{}   Vault 143 Hotdish x{}   Scrap x{}   Frostfang pelts {}\n",
            inv.stimpaks, inv.radaway, inv.hotdish, inv.scrap, inv.pelts
        );
        out += &format!(
            "\nCRAFTING\n  Frostfang coat (C at a shelter): {}\n  Weapon upgrades (B at a workbench beside a fish house):\n",
            if inv.has_frostfang_coat { "WORN".to_string() } else { format!("{} of {} pelts", inv.pelts, Inventory::PELTS_FOR_COAT) }
        );
        for kind in WeaponKind::ALL {
            if !game.arsenal.owned[kind.slot()] || kind.ammo().is_none() {
                continue;
            }
            let w = game.arsenal.get(kind);
            match w.next_upgrade() {
                Some(up) => out += &format!("    {:<18} next: {} ({}), {} scrap\n", kind.name(), up.name(), up.describe(), up.scrap_cost()),
                None => out += &format!("    {:<18} fully upgraded\n", kind.name()),
            }
        }
        out += "\nTip: containers (E) and creatures hold scrap. Weapons are hidden in the Bullseye-Mart,\nat the Golden Atomic Mills, and in a fish house tackle box.";
        t.0 = out;
    }
}

/// Click a tab to open it.
fn tab_buttons(
    mut st: ResMut<PipState>,
    mut sfx: ResMut<SfxQueue>,
    mut buttons: Query<(&Interaction, &TabButton, &mut BackgroundColor, &mut BorderColor)>,
) {
    for (interaction, tab, mut bg, mut border) in &mut buttons {
        if *interaction == Interaction::Pressed && st.tab != tab.0 {
            st.tab = tab.0;
            sfx.play(Sound::UiTab);
        }
        let active = st.tab == tab.0;
        bg.0 = if active { Color::srgba(0.45, 1.0, 0.45, 0.22) } else { Color::NONE };
        border.0 = if active { GREEN } else { DIM };
    }
}
