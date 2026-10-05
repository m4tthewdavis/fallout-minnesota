//! The Pip-Boy 3000, New Vegas style. Tab or M raises your wrist into view
//! (the game pauses) and the amber CRT powers on: scanlines, curved glass,
//! flicker, a rolling scan bar, and a burst of static whenever the page
//! changes.
//!
//! * STATS: STATUS (the Vault 143 mascot, who shivers when you're cold, wears
//!   a bandage when you're hurt and glows when you're irradiated, with your
//!   condition) and S.P.E.C.I.A.L.
//! * ITEMS: WEAPONS, APPAREL and AID (Enter uses the selected aid item).
//! * DATA: WORLD MAP (the fogged terrain map with markers and a cursor) and
//!   NOTES (objectives, places, settings).
//!
//! Keys: 1 2 3 or the buttons under the screen pick STATS / ITEMS / DATA,
//! Q and E (or clicking) change the page along the bottom, W/S or the arrows
//! move through lists. On the map: wheel zoom, drag or WASD pan, C centres.
//! The map drawing and fog rules are in `sim::mapdata`.

use bevy::input::mouse::{AccumulatedMouseMotion, AccumulatedMouseScroll};
use bevy::image::ImageSampler;
use bevy::prelude::*;
use bevy::render::render_asset::RenderAssetUsages;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use bevy::ui::widget::NodeImageMode;
use bevy::ui::{FocusPolicy, RelativeCursorPosition};
use bevy::window::PrimaryWindow;

use crate::assets::GameAssets;
use crate::audio::{AudioSettings, GeigerOn};
use crate::player::{set_grab, use_aid, Player};
use crate::sim::combat::{Upgrade, WeaponKind};
use crate::sim::mapdata::{self, Fog, Kind};
use crate::menu::{SaveRequest, SlotSummaries};
use crate::sim::menu::{slot_name, AUTOSAVE, QUICKSAVE, SLOTS};
use crate::sim::pipnav::{Layout, Main, Nav, Page};
use crate::sim::progress;
use crate::sim::survival::{Aid, Exposure, Inventory, Survival};
use crate::sim::synth::Sound;
use crate::sim::terrain::{self, HALF_SIZE};
use crate::sim::weather::Phase;
use crate::state::{ClockRes, Game, Messages, PipOpen, SfxQueue, TreePositions, WeatherRes};

/// New Vegas amber.
const AMBER: Color = Color::srgb(1.0, 0.72, 0.3);
const AMBER_DIM: Color = Color::srgba(1.0, 0.72, 0.3, 0.45);
const AMBER_FAINT: Color = Color::srgba(1.0, 0.72, 0.3, 0.14);
const SCREEN_BG: Color = Color::srgb(0.075, 0.042, 0.012);

/// The device art is 1500x1000 px, drawn at 1000x667 logical px.
const DEVICE_W: f32 = 1000.0;
const DEVICE_H: f32 = 667.0;
/// Where the screen window sits in the device (logical px).
const SCREEN_X: f32 = 150.0;
const SCREEN_Y: f32 = 70.0;
const SCREEN_W: f32 = 640.0;
const SCREEN_H: f32 = 460.0;
/// The square world-map view.
const VIEW: f32 = 330.0;
const MAP_PX: usize = 512;
const REVEAL_RADIUS: f32 = 42.0;
/// Which way UI rotation turns relative to a compass heading (checked on screen).
const ARROW_SIGN: f32 = -1.0;
/// Rows in the list pages.
const ROWS: usize = 8;

/// Seconds to raise the Pip-Boy into view, and to lower it.
const RAISE_SECS: f32 = 0.38;
const LOWER_SECS: f32 = 0.26;

#[derive(Resource)]
pub(crate) struct PipState {
    /// Which page you're on and what's selected (rules in `sim::pipnav`).
    nav: Nav,
    /// Map zoom (1 = whole map fits the view) and the world point at its centre.
    zoom: f32,
    center: Vec2,
    pub(crate) fog: Fog,
    base: Option<Handle<Image>>,
    fog_image: Handle<Image>,
    /// The fog picture needs re-uploading.
    pub(crate) dirty: bool,
    /// 0 = lowered out of sight, 1 = raised; and where it's heading.
    raise: f32,
    raising: bool,
    /// Real seconds since the screen powered on.
    power: f32,
    /// Static burst left (seconds) after a page change.
    burst: f32,
    static_frames: [Handle<Image>; 4],
}

#[derive(Component)]
struct PipRoot;
#[derive(Component)]
struct Device;
#[derive(Component)]
struct ScreenCover;
#[derive(Component)]
struct Beam;
#[derive(Component)]
struct StaticOverlay;
#[derive(Component)]
struct ScanBar;
#[derive(Component)]
struct Flicker;
#[derive(Component)]
struct MainTitle;
#[derive(Component)]
struct HeaderInfo;
#[derive(Component)]
struct SubTab(usize);
#[derive(Component)]
struct MainButton(Main);
#[derive(Component)]
struct MainLamp(Main);
#[derive(Component)]
struct LayoutNode(Layout);
#[derive(Component)]
struct ListRow(usize);
#[derive(Component)]
struct DetailText;
#[derive(Component)]
struct DetailMascot;
#[derive(Component)]
struct StatusMascot;
#[derive(Component)]
struct StatusGlow;
#[derive(Component)]
struct StatusBar(usize);
#[derive(Component)]
struct StatusText;
#[derive(Component)]
struct StatusSide;
#[derive(Component)]
struct NotesText;
#[derive(Component)]
struct MapView;
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
struct MapCursor;
#[derive(Component)]
struct CursorLabel;
#[derive(Component)]
struct InfoText;
#[derive(Component)]
struct Footer;

/// The mascot's poses.
#[derive(Resource)]
struct MascotArt {
    idle: Handle<Image>,
    cold: Handle<Image>,
    hurt: Handle<Image>,
}

pub struct PipboyPlugin;

impl Plugin for PipboyPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, build_ui)
            .add_systems(Update, auto_open.before(toggle_pipboy))
            .add_systems(
                Update,
                (
                    update_fog,
                    toggle_pipboy,
                    animate_device.after(toggle_pipboy),
                    close_on_load,
                    hide_weapon,
                    (pipboy_input, buttons, update_header, update_status, update_list, update_notes, update_map, show_layout).chain().run_if(is_open),
                ),
            );
    }
}

/// Screenshot mode: FMN_PIP=map|stats|special|weapons|apparel|aid|notes (or
/// the old inventory) opens the Pip-Boy after a moment (FMN_PIP_ZOOM and
/// FMN_PIP_REVEAL=1 tune the map, the latter lifting the fog).
fn auto_open(real: Res<Time<Real>>, mut stage: Local<u8>, mut keys: ResMut<ButtonInput<KeyCode>>, mut st: ResMut<PipState>, open: Res<PipOpen>) {
    let Ok(which) = std::env::var("FMN_PIP") else { return };
    let t = real.elapsed_secs();
    if *stage == 0 && t > 0.8 {
        *stage = 1;
        let page = match which.as_str() {
            "stats" | "status" => Page::Status,
            "special" => Page::Special,
            "inventory" | "weapons" => Page::Weapons,
            "apparel" => Page::Apparel,
            "aid" => Page::Aid,
            "notes" => Page::Notes,
            "saves" => Page::Saves,
            _ => Page::Map,
        };
        st.nav.go_to(page);
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

fn abs(left: f32, top: f32, w: f32, h: f32) -> Node {
    Node {
        position_type: PositionType::Absolute,
        left: Val::Px(left),
        top: Val::Px(top),
        width: Val::Px(w),
        height: Val::Px(h),
        ..default()
    }
}

fn fill() -> Node {
    Node {
        position_type: PositionType::Absolute,
        left: Val::Px(0.0),
        top: Val::Px(0.0),
        width: Val::Percent(100.0),
        height: Val::Percent(100.0),
        ..default()
    }
}

fn build_ui(mut commands: Commands, assets: Res<GameAssets>, server: Res<AssetServer>, mut images: ResMut<Assets<Image>>) {
    let fog = Fog::new();
    let fog_image = images.add(rgba_image(mapdata::FOG_CELLS as u32, mapdata::FOG_CELLS as u32, fog.overlay()));
    let arrow = images.add(rgba_image(32, 32, mapdata::arrow_icon(32)));
    let static_frames = [0, 1, 2, 3].map(|k| server.load(format!("ui/pip_static_{k}.png")));
    commands.insert_resource(PipState {
        nav: Nav::default(),
        zoom: 1.0,
        center: Vec2::ZERO,
        fog,
        base: None,
        fog_image: fog_image.clone(),
        dirty: false,
        raise: 0.0,
        raising: false,
        power: 0.0,
        burst: 0.0,
        static_frames: static_frames.clone(),
    });
    let mascot = MascotArt {
        idle: server.load("ui/mascot_idle.png"),
        cold: server.load("ui/mascot_cold.png"),
        hurt: server.load("ui/mascot_hurt.png"),
    };
    let glow = server.load::<Image>("ui/mascot_glow.png");
    let frame = server.load::<Image>("ui/pip_frame.png");
    let crt = server.load::<Image>("ui/pip_crt.png");

    let font = |size: f32| TextFont {
        font: assets.font.clone(),
        font_size: size,
        ..default()
    };
    let img = |h: &Handle<Image>, color: Color| ImageNode {
        image: h.clone(),
        color,
        image_mode: NodeImageMode::Stretch,
        ..default()
    };

    // Root: darkens the paused world and centres the device.
    let root = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                overflow: Overflow::clip(),
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.0)),
            GlobalZIndex(100),
            Visibility::Hidden,
            PipRoot,
        ))
        .id();
    let device = commands
        .spawn((
            Node {
                width: Val::Px(DEVICE_W),
                height: Val::Px(DEVICE_H),
                top: Val::Px(DEVICE_H),
                flex_shrink: 0.0,
                ..default()
            },
            Device,
            ChildOf(root),
        ))
        .id();

    // ---------------- The screen ----------------
    let screen = commands
        .spawn((
            Node {
                overflow: Overflow::clip(),
                flex_direction: FlexDirection::Column,
                padding: UiRect::new(Val::Px(26.0), Val::Px(26.0), Val::Px(18.0), Val::Px(12.0)),
                ..abs(SCREEN_X, SCREEN_Y, SCREEN_W, SCREEN_H)
            },
            BackgroundColor(SCREEN_BG),
            ChildOf(device),
        ))
        .id();
    commands.entity(screen).with_children(|s| {
        // Header: the top tab's name on a rule, and your vitals.
        s.spawn((
            Node {
                justify_content: JustifyContent::SpaceBetween,
                align_items: AlignItems::FlexEnd,
                border: UiRect::bottom(Val::Px(2.0)),
                padding: UiRect::bottom(Val::Px(3.0)),
                ..default()
            },
            BorderColor(AMBER),
        ))
        .with_children(|h| {
            h.spawn((Text::new("STATS"), font(24.0), TextColor(AMBER), MainTitle));
            h.spawn((Text::new(""), font(14.0), TextColor(AMBER), HeaderInfo));
        });

        // Page area.
        s.spawn(Node {
            flex_grow: 1.0,
            margin: UiRect::vertical(Val::Px(10.0)),
            ..default()
        })
        .with_children(|area| {
            // ---- STATUS ----
            area.spawn((Node { width: Val::Percent(100.0), height: Val::Percent(100.0), column_gap: Val::Px(14.0), ..default() }, LayoutNode(Layout::Status)))
                .with_children(|p| {
                    p.spawn(Node { width: Val::Px(170.0), flex_direction: FlexDirection::Column, ..default() })
                        .with_children(|c| {
                            c.spawn((Text::new(""), font(14.0), TextColor(AMBER), StatusSide));
                        });
                    p.spawn(Node {
                        width: Val::Px(190.0),
                        flex_direction: FlexDirection::Column,
                        align_items: AlignItems::Center,
                        ..default()
                    })
                    .with_children(|c| {
                        c.spawn(Node { width: Val::Px(160.0), height: Val::Px(224.0), ..default() }).with_children(|m| {
                            m.spawn((fill(), img(&glow, AMBER.with_alpha(0.0)), StatusGlow));
                            m.spawn((fill(), img(&mascot.idle, AMBER), StatusMascot));
                        });
                        for (i, label) in ["HP", "HEAT", "RAD"].into_iter().enumerate() {
                            c.spawn(Node { align_items: AlignItems::Center, column_gap: Val::Px(6.0), margin: UiRect::top(Val::Px(4.0)), ..default() }).with_children(|row| {
                                row.spawn((Text::new(label), font(13.0), TextColor(AMBER), Node { width: Val::Px(38.0), ..default() }));
                                row.spawn((Node { width: Val::Px(120.0), height: Val::Px(9.0), border: UiRect::all(Val::Px(1.0)), padding: UiRect::all(Val::Px(1.0)), ..default() }, BorderColor(AMBER_DIM)))
                                    .with_children(|b| {
                                        b.spawn((Node { width: Val::Percent(100.0), height: Val::Percent(100.0), ..default() }, BackgroundColor(AMBER), StatusBar(i)));
                                    });
                            });
                        }
                    });
                    p.spawn((Text::new(""), font(14.0), TextColor(AMBER), Node { flex_grow: 1.0, ..default() }, StatusText));
                });

            // ---- Lists (S.P.E.C.I.A.L., WEAPONS, APPAREL, AID) ----
            area.spawn((Node { width: Val::Percent(100.0), height: Val::Percent(100.0), column_gap: Val::Px(16.0), display: Display::None, ..default() }, LayoutNode(Layout::List)))
                .with_children(|p| {
                    p.spawn(Node { width: Val::Px(250.0), flex_direction: FlexDirection::Column, row_gap: Val::Px(2.0), ..default() }).with_children(|col| {
                        for i in 0..ROWS {
                            col.spawn((
                                Button,
                                Node { padding: UiRect::axes(Val::Px(8.0), Val::Px(3.0)), border: UiRect::all(Val::Px(1.0)), ..default() },
                                BorderColor(Color::NONE),
                                BackgroundColor(Color::NONE),
                                ListRow(i),
                            ))
                            .with_children(|r| {
                                r.spawn((Text::new(""), font(15.0), TextColor(AMBER)));
                            });
                        }
                    });
                    p.spawn(Node { flex_grow: 1.0, flex_direction: FlexDirection::Column, ..default() }).with_children(|col| {
                        col.spawn((Node { width: Val::Px(110.0), height: Val::Px(154.0), align_self: AlignSelf::Center, ..default() }, img(&mascot.idle, AMBER), DetailMascot));
                        col.spawn((Text::new(""), font(14.0), TextColor(AMBER), DetailText));
                    });
                });

            // ---- WORLD MAP ----
            area.spawn((Node { width: Val::Percent(100.0), height: Val::Percent(100.0), column_gap: Val::Px(14.0), display: Display::None, ..default() }, LayoutNode(Layout::Map)))
                .with_children(|page| {
                    page.spawn((
                        Node { width: Val::Px(VIEW), height: Val::Px(VIEW), overflow: Overflow::clip(), border: UiRect::all(Val::Px(1.0)), ..default() },
                        BorderColor(AMBER_DIM),
                        BackgroundColor(Color::srgb(0.03, 0.02, 0.005)),
                        RelativeCursorPosition::default(),
                        MapView,
                    ))
                    .with_children(|view| {
                        view.spawn((Node { position_type: PositionType::Absolute, width: Val::Px(VIEW), height: Val::Px(VIEW), ..default() }, MapWorld))
                            .with_children(|world| {
                                world.spawn((fill(), ImageNode::default(), MapBase));
                                world.spawn((fill(), img(&fog_image, Color::WHITE)));
                                for (i, lm) in mapdata::landmarks().iter().enumerate() {
                                    let (u, v) = mapdata::to_uv(lm.x, lm.z);
                                    // New Vegas style markers: hollow diamonds, filled for the vault.
                                    let filled = lm.kind == Kind::Vault;
                                    world.spawn((
                                        Node {
                                            position_type: PositionType::Absolute,
                                            left: Val::Percent(u * 100.0),
                                            top: Val::Percent(v * 100.0),
                                            width: Val::Px(10.0),
                                            height: Val::Px(10.0),
                                            margin: UiRect::all(Val::Px(-5.0)),
                                            border: UiRect::all(Val::Px(2.0)),
                                            ..default()
                                        },
                                        BorderColor(AMBER),
                                        BackgroundColor(if filled { AMBER } else { SCREEN_BG.with_alpha(0.6) }),
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
                                                margin: UiRect { left: Val::Px(9.0), top: Val::Px(-8.0), ..default() },
                                                ..default()
                                            },
                                            Visibility::Hidden,
                                            LandmarkLabel(i),
                                        ))
                                        .with_children(|label| {
                                            label.spawn((Text::new(lm.name), font(12.0), TextColor(AMBER)));
                                        });
                                }
                                world.spawn((
                                    Node {
                                        position_type: PositionType::Absolute,
                                        left: Val::Percent(50.0),
                                        top: Val::Percent(50.0),
                                        width: Val::Px(22.0),
                                        height: Val::Px(22.0),
                                        margin: UiRect::all(Val::Px(-11.0)),
                                        ..default()
                                    },
                                    ImageNode { image: arrow.clone(), color: AMBER, image_mode: NodeImageMode::Stretch, ..default() },
                                    PlayerMarker,
                                ));
                            });
                        // The map cursor: crosshair lines and a readout of what's under it.
                        view.spawn((Node { position_type: PositionType::Absolute, width: Val::Px(VIEW), height: Val::Px(1.0), ..default() }, BackgroundColor(AMBER_FAINT), MapCursor, FocusPolicy::Pass));
                        view.spawn((Node { position_type: PositionType::Absolute, width: Val::Px(1.0), height: Val::Px(VIEW), ..default() }, BackgroundColor(AMBER_FAINT), MapCursor, FocusPolicy::Pass));
                        view.spawn((Node { position_type: PositionType::Absolute, ..default() }, Text::new(""), font(12.0), TextColor(AMBER), CursorLabel));
                    });
                    page.spawn((Text::new(""), font(13.0), TextColor(AMBER), Node { flex_grow: 1.0, ..default() }, InfoText));
                });

            // ---- NOTES ----
            area.spawn((Node { width: Val::Percent(100.0), height: Val::Percent(100.0), display: Display::None, ..default() }, LayoutNode(Layout::Notes)))
                .with_children(|p| {
                    p.spawn((Text::new(""), font(14.0), TextColor(AMBER), NotesText));
                });
        });

        // Footer: the pages of this tab, the selected one boxed.
        s.spawn((
            Node {
                column_gap: Val::Px(18.0),
                border: UiRect::top(Val::Px(2.0)),
                padding: UiRect::top(Val::Px(5.0)),
                align_items: AlignItems::Center,
                ..default()
            },
            BorderColor(AMBER),
            Footer,
        ))
        .with_children(|f| {
            for i in 0..3 {
                f.spawn((
                    Button,
                    Node { padding: UiRect::axes(Val::Px(8.0), Val::Px(2.0)), border: UiRect::all(Val::Px(1.0)), ..default() },
                    BorderColor(Color::NONE),
                    BackgroundColor(Color::NONE),
                    SubTab(i),
                ))
                .with_children(|b| {
                    b.spawn((Text::new(""), font(15.0), TextColor(AMBER)));
                });
            }
        });

        // ---- CRT effects, on top of the content ----
        s.spawn((
            fill(),
            ImageNode {
                image: assets.scanlines.clone(),
                color: Color::srgba(0.0, 0.0, 0.0, 0.85),
                image_mode: NodeImageMode::Tiled { tile_x: true, tile_y: true, stretch_value: 1.0 },
                ..default()
            },
            FocusPolicy::Pass,
        ));
        s.spawn((
            Node { position_type: PositionType::Absolute, left: Val::Px(0.0), width: Val::Percent(100.0), height: Val::Px(46.0), ..default() },
            BackgroundColor(AMBER.with_alpha(0.03)),
            ScanBar,
            FocusPolicy::Pass,
        ));
        s.spawn((
            fill(),
            ImageNode {
                image: static_frames[0].clone(),
                color: Color::srgba(1.0, 0.8, 0.5, 0.05),
                image_mode: NodeImageMode::Tiled { tile_x: true, tile_y: true, stretch_value: 1.0 },
                ..default()
            },
            StaticOverlay,
            FocusPolicy::Pass,
        ));
        s.spawn((fill(), BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.0)), Flicker, FocusPolicy::Pass));
        s.spawn((fill(), img(&crt, Color::WHITE), FocusPolicy::Pass));
        // Power-on: the screen starts black, a bright line opens up into the picture.
        s.spawn((fill(), BackgroundColor(Color::BLACK), ScreenCover, FocusPolicy::Pass));
        s.spawn((
            Node { position_type: PositionType::Absolute, left: Val::Px(0.0), top: Val::Percent(50.0), width: Val::Percent(100.0), height: Val::Px(2.0), ..default() },
            BackgroundColor(Color::srgb(1.0, 0.9, 0.7)),
            Visibility::Hidden,
            Beam,
            FocusPolicy::Pass,
        ));
    });

    // ---------------- The casing over the screen ----------------
    commands.spawn((abs(0.0, 0.0, DEVICE_W, DEVICE_H), img(&frame, Color::WHITE), FocusPolicy::Pass, ChildOf(device)));
    // The STATS / ITEMS / DATA buttons under the screen, with a lamp over the active one.
    for (i, main) in Main::ALL.into_iter().enumerate() {
        let x = 220.0 + i as f32 * 193.3;
        commands.spawn((Button, abs(x, 566.7, 133.3, 43.3), BackgroundColor(Color::NONE), MainButton(main), ChildOf(device)));
        commands.spawn((abs(x + 50.0, 556.0, 33.0, 5.0), BackgroundColor(AMBER), MainLamp(main), ChildOf(device)));
    }
    commands.spawn((
        Text::new("TAB close   1 2 3 tabs   Q E pages   W S select   ENTER use / save   L load"),
        font(13.0),
        TextColor(Color::srgba(1.0, 0.9, 0.7, 0.75)),
        Node { position_type: PositionType::Absolute, left: Val::Px(160.0), top: Val::Px(DEVICE_H - 34.0), ..default() },
        ChildOf(device),
    ));
    commands.insert_resource(mascot);
}

/// Reveal the map around you, find landmarks, and refresh the fog picture.
fn update_fog(mut st: ResMut<PipState>, mut images: ResMut<Assets<Image>>, mut msgs: ResMut<Messages>, mut sfx: ResMut<SfxQueue>, player: Query<&Transform, With<Player>>) {
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

/// Loading a save from the Pip-Boy puts it away and returns you to the game.
fn close_on_load(
    mut requests: EventReader<SaveRequest>,
    mut open: ResMut<PipOpen>,
    mut st: ResMut<PipState>,
    mut vtime: ResMut<Time<Virtual>>,
    mut windows: Query<&mut Window, With<PrimaryWindow>>,
) {
    let loading = requests.read().any(|r| matches!(r, SaveRequest::Load(_)));
    if loading && open.0 {
        open.0 = false;
        st.raising = false;
        vtime.unpause();
        if let Ok(mut w) = windows.single_mut() {
            set_grab(&mut w, true);
        }
    }
}

/// Switch page, with a burst of static.
fn go_to(st: &mut PipState, sfx: &mut SfxQueue, page: Page) {
    if st.nav.go_to(page) {
        st.burst = 0.22;
        sfx.play(Sound::UiTab);
        sfx.play(Sound::PipStatic);
    }
}

/// The static and sound that go with a page change you've already made.
fn page_changed(st: &mut PipState, sfx: &mut SfxQueue) {
    st.burst = 0.22;
    sfx.play(Sound::UiTab);
    sfx.play(Sound::PipStatic);
}

fn toggle_pipboy(
    keys: Res<ButtonInput<KeyCode>>,
    mut open: ResMut<PipOpen>,
    paused: Res<crate::state::Paused>,
    game: Res<Game>,
    mut st: ResMut<PipState>,
    trees: Res<TreePositions>,
    mut images: ResMut<Assets<Image>>,
    mut vtime: ResMut<Time<Virtual>>,
    mut sfx: ResMut<SfxQueue>,
    mut windows: Query<&mut Window, With<PrimaryWindow>>,
    mut base: Query<&mut ImageNode, With<MapBase>>,
    player: Query<&Transform, With<Player>>,
) {
    if paused.0 {
        return;
    }
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
    if open.0 {
        // Draw the map the first time it is needed.
        if st.base.is_none() {
            let started = std::time::Instant::now();
            let mut data = mapdata::render_base(MAP_PX, &trees.0);
            // The map is drawn in Pip-Boy green; recolour it for the amber screen.
            for px in data.as_chunks_mut::<4>().0 {
                let l = px[0].max(px[1]).max(px[2]) as f32;
                px[0] = l as u8;
                px[1] = (l * 0.72) as u8;
                px[2] = (l * 0.3) as u8;
            }
            info!("pip-boy map drawn in {:.2}s", started.elapsed().as_secs_f32());
            st.base = Some(images.add(rgba_image(MAP_PX as u32, MAP_PX as u32, data)));
        }
        if let (Some(handle), Ok(mut node)) = (st.base.clone(), base.single_mut()) {
            node.image = handle;
            node.image_mode = NodeImageMode::Stretch;
        }
        if let Ok(p) = player.single() {
            let (zoom, at) = (st.zoom, Vec2::new(p.translation.x, p.translation.z));
            st.center = clamp_center(at, zoom);
        }
        st.raising = true;
        st.power = 0.0;
        vtime.pause();
        if let Ok(mut w) = windows.single_mut() {
            set_grab(&mut w, false);
        }
        sfx.play(Sound::PipOn);
    } else {
        // The game resumes as your arm drops.
        st.raising = false;
        vtime.unpause();
        if let Ok(mut w) = windows.single_mut() {
            set_grab(&mut w, true);
        }
        sfx.play(Sound::PipOff);
    }
}

/// Your weapon goes down while your wrist is up. (Hide the model, not its
/// camera: the UI is drawn by that camera.)
fn hide_weapon(st: Res<PipState>, mut gun: Query<&mut Visibility, Or<(With<crate::gun::GunModel>, With<crate::gun::ArmsRig>)>>) {
    let want = if st.raise < 0.5 { Visibility::Inherited } else { Visibility::Hidden };
    for mut v in &mut gun {
        if *v != want {
            *v = want;
        }
    }
}

fn ease_out(t: f32) -> f32 {
    1.0 - (1.0 - t).powi(3)
}

/// Raise and lower the device, power the screen on, and run the CRT effects.
fn animate_device(
    real: Res<Time<Real>>,
    mut st: ResMut<PipState>,
    mut root: Query<(&mut Visibility, &mut BackgroundColor), (With<PipRoot>, Without<Beam>)>,
    mut device: Query<(&mut Node, &mut Transform), (With<Device>, Without<ScanBar>, Without<Beam>)>,
    mut cover: Query<&mut BackgroundColor, (With<ScreenCover>, Without<PipRoot>, Without<Flicker>)>,
    mut beam: Query<(&mut Node, &mut Visibility), (With<Beam>, Without<Device>, Without<PipRoot>, Without<ScanBar>)>,
    mut scan: Query<&mut Node, (With<ScanBar>, Without<Device>, Without<Beam>)>,
    mut noise: Query<&mut ImageNode, With<StaticOverlay>>,
    mut flicker: Query<&mut BackgroundColor, (With<Flicker>, Without<PipRoot>, Without<ScreenCover>)>,
    mut seed: Local<u32>,
) {
    // Screenshot mode renders a frame every second or so: skip straight to the end.
    let dt = if std::env::var("FMN_SHOT").is_ok() { 1.0 } else { real.delta_secs().min(0.1) };
    let t = real.elapsed_secs();
    // Raise or lower.
    let target = if st.raising { 1.0 } else { 0.0 };
    let rate = if st.raising { 1.0 / RAISE_SECS } else { 1.0 / LOWER_SECS };
    st.raise = if st.raise < target { (st.raise + dt * rate).min(target) } else { (st.raise - dt * rate).max(target) };
    let Ok((mut vis, mut dim)) = root.single_mut() else { return };
    if st.raise <= 0.0 && !st.raising {
        if *vis != Visibility::Hidden {
            *vis = Visibility::Hidden;
        }
        return;
    }
    *vis = Visibility::Visible;
    let r = ease_out(st.raise);
    dim.0 = Color::srgba(0.0, 0.0, 0.0, 0.62 * r);
    if let Ok((mut node, mut tf)) = device.single_mut() {
        // Up from below the screen, tilting level as it comes.
        node.top = Val::Px((1.0 - r) * DEVICE_H * 1.1);
        tf.rotation = Quat::from_rotation_z((1.0 - r) * 0.12);
    }

    // Power-on: 0.1 s dark, a bright line that opens over 0.12 s, then the picture
    // fades up with a stutter.
    if st.raising {
        st.power += dt;
    }
    let p = st.power - 0.18;
    if let Ok(mut c) = cover.single_mut() {
        let a = if !st.raising {
            0.0
        } else if p < 0.12 {
            1.0
        } else {
            let k = ((p - 0.12) / 0.3).clamp(0.0, 1.0);
            let stutter = if k < 1.0 && ((t * 37.0).sin() > 0.6) { 0.25 } else { 0.0 };
            ((1.0 - k) + stutter).clamp(0.0, 1.0)
        };
        c.0 = Color::srgba(0.0, 0.0, 0.0, a);
    }
    if let Ok((mut n, mut v)) = beam.single_mut() {
        if st.raising && (0.0..0.12).contains(&p) {
            *v = Visibility::Inherited;
            let k = p / 0.12;
            n.height = Val::Px(2.0 + k * SCREEN_H * 0.9);
            n.top = Val::Px(SCREEN_H * 0.5 - (1.0 + k * SCREEN_H * 0.45));
        } else {
            *v = Visibility::Hidden;
        }
    }
    // A soft bright band rolls down the screen every few seconds.
    if let Ok(mut n) = scan.single_mut() {
        let y = (t * 0.25).fract() * (SCREEN_H + 120.0) - 60.0;
        n.top = Val::Px(y);
    }
    // Static: always a faint crawl, a burst after page changes.
    st.burst = (st.burst - dt).max(0.0);
    *seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
    if let Ok(mut img) = noise.single_mut() {
        let frame = ((t * 24.0) as usize + (*seed >> 30) as usize) % 4;
        img.image = st.static_frames[frame].clone();
        let a = 0.045 + st.burst / 0.22 * 0.55;
        img.color = Color::srgba(1.0, 0.8, 0.5, a);
    }
    // Faint brightness flicker.
    if let Ok(mut f) = flicker.single_mut() {
        let jitter = ((*seed >> 8) & 0xFF) as f32 / 255.0;
        let a = if jitter > 0.93 { 0.08 } else { 0.015 * (t * 60.0).sin().abs() };
        f.0 = Color::srgba(0.0, 0.0, 0.0, a);
    }
}

fn pipboy_input(
    time: Res<Time<Real>>,
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    motion: Res<AccumulatedMouseMotion>,
    scroll: Res<AccumulatedMouseScroll>,
    mut st: ResMut<PipState>,
    mut sfx: ResMut<SfxQueue>,
    mut game: ResMut<Game>,
    mut msgs: ResMut<Messages>,
    slots: Res<SlotSummaries>,
    mut saves: EventWriter<SaveRequest>,
    player: Query<&Transform, With<Player>>,
) {
    for (key, main) in [(KeyCode::Digit1, Main::Stats), (KeyCode::Digit2, Main::Items), (KeyCode::Digit3, Main::Data)] {
        if keys.just_pressed(key) && st.nav.switch_main(main) {
            page_changed(&mut st, &mut sfx);
        }
    }
    if keys.just_pressed(KeyCode::KeyQ) && st.nav.step_page(-1) {
        page_changed(&mut st, &mut sfx);
    }
    if keys.just_pressed(KeyCode::KeyE) && st.nav.step_page(1) {
        page_changed(&mut st, &mut sfx);
    }

    if st.nav.page.layout() == Layout::List {
        let count = list_items(st.nav.page, &game, &slots).len();
        let up = keys.just_pressed(KeyCode::KeyW) || keys.just_pressed(KeyCode::ArrowUp);
        let down = keys.just_pressed(KeyCode::KeyS) || keys.just_pressed(KeyCode::ArrowDown);
        if (up && st.nav.move_selection(count, -1)) || (down && st.nav.move_selection(count, 1)) {
            sfx.play(Sound::PipScroll);
        }
        st.nav.clamp_selection(count);
        if st.nav.page == Page::Aid && (keys.just_pressed(KeyCode::Enter) || keys.just_pressed(KeyCode::Space)) {
            let aid = Aid::ALL[st.nav.selected_row().min(2)];
            if use_aid(aid, &mut game, &mut msgs) {
                sfx.play(Sound::PickupMed);
            } else {
                sfx.play(Sound::DryClick);
            }
        }
        if st.nav.page == Page::Saves {
            let slot = st.nav.selected_row().min(SLOTS - 1);
            if keys.just_pressed(KeyCode::Enter) || keys.just_pressed(KeyCode::Space) {
                if slot == AUTOSAVE {
                    sfx.play(Sound::DryClick);
                } else {
                    saves.write(SaveRequest::Save(slot));
                    sfx.play(Sound::Craft);
                }
            }
            if keys.just_pressed(KeyCode::KeyL) {
                if slots.0[slot].is_some() {
                    saves.write(SaveRequest::Load(slot));
                } else {
                    sfx.play(Sound::DryClick);
                }
            }
        }
    }

    if st.nav.page != Page::Map {
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
    // Pan: drag with the mouse, or WASD / arrows.
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

/// Mouse: the buttons under the screen, the page tabs and the list rows.
fn buttons(
    mut st: ResMut<PipState>,
    mut sfx: ResMut<SfxQueue>,
    game: Res<Game>,
    slots: Res<SlotSummaries>,
    mains: Query<(&Interaction, &MainButton), Changed<Interaction>>,
    subs: Query<(&Interaction, &SubTab), (Changed<Interaction>, Without<MainButton>)>,
    rows: Query<(&Interaction, &ListRow), (Changed<Interaction>, Without<SubTab>, Without<MainButton>)>,
) {
    for (interaction, b) in &mains {
        if *interaction == Interaction::Pressed && st.nav.switch_main(b.0) {
            page_changed(&mut st, &mut sfx);
        }
    }
    for (interaction, tab) in &subs {
        if let (Interaction::Pressed, Some(page)) = (*interaction, st.nav.page_in_tab(tab.0)) {
            go_to(&mut st, &mut sfx, page);
        }
    }
    for (interaction, row) in &rows {
        if matches!(interaction, Interaction::Hovered | Interaction::Pressed) && st.nav.page.layout() == Layout::List {
            let count = list_items(st.nav.page, &game, &slots).len();
            if st.nav.select_row(row.0, count) {
                sfx.play(Sound::PipScroll);
            }
        }
    }
}

/// Show the right page layout; mark the active top button and page tab.
fn show_layout(
    st: Res<PipState>,
    mut layouts: Query<(&mut Node, &LayoutNode)>,
    mut lamps: Query<(&MainLamp, &mut BackgroundColor), Without<SubTab>>,
    mut subs: Query<(&SubTab, &mut Node, &mut BorderColor, &mut BackgroundColor, &Children), (Without<LayoutNode>, Without<MainLamp>)>,
    mut texts: Query<&mut Text, Without<MainTitle>>,
    mut title: Query<&mut Text, With<MainTitle>>,
) {
    let layout = st.nav.page.layout();
    for (mut node, l) in &mut layouts {
        let want = if l.0 == layout { Display::Flex } else { Display::None };
        if node.display != want {
            node.display = want;
        }
    }
    for (lamp, mut bg) in &mut lamps {
        bg.0 = if lamp.0 == st.nav.page.main() { AMBER } else { Color::srgba(0.2, 0.15, 0.05, 0.8) };
    }
    let pages = st.nav.page.main().pages();
    for (tab, mut node, mut border, mut bg, children) in &mut subs {
        let Some(page) = pages.get(tab.0) else {
            node.display = Display::None;
            continue;
        };
        node.display = Display::Flex;
        let active = *page == st.nav.page;
        border.0 = if active { AMBER } else { Color::NONE };
        bg.0 = if active { AMBER_FAINT } else { Color::NONE };
        if let Some(&child) = children.first() {
            if let Ok(mut t) = texts.get_mut(child) {
                if t.0 != page.label() {
                    t.0 = page.label().to_string();
                }
            }
        }
    }
    if let Ok(mut t) = title.single_mut() {
        t.0 = st.nav.page.main().label().to_string();
    }
}

fn bar(v: f32, width: usize) -> String {
    let n = (v.clamp(0.0, 1.0) * width as f32).round() as usize;
    format!("[{}{}]", "#".repeat(n), "-".repeat(width - n))
}

fn total_upgrades(game: &Game) -> usize {
    WeaponKind::ALL.iter().filter(|k| game.arsenal.owned[k.slot()]).map(|k| Upgrade::ALL.iter().filter(|u| game.arsenal.get(*k).has_upgrade(**u)).count()).sum()
}

/// Header: level and XP, vitals, scrap and the date.
fn update_header(st: Res<PipState>, game: Res<Game>, clock: Res<ClockRes>, mut q: Query<&mut Text, With<HeaderInfo>>) {
    let Ok(mut t) = q.single_mut() else { return };
    let s = &game.survival;
    let places = st.fog.landmarks_found.iter().filter(|f| **f).count();
    let xp = progress::experience(game.kills, places, total_upgrades(&game), game.inv.has_frostfang_coat);
    let (lvl, into, span) = progress::level(xp);
    t.0 = format!(
        "LVL {}   HP {:.0}/{:.0}   HEAT {:.0}%   RAD {:.0}   XP {}/{}   DAY {} {}",
        lvl,
        s.health,
        s.max_health(),
        s.body_heat,
        s.rads,
        into,
        span,
        clock.0.day,
        clock.0.label()
    );
}

/// STATUS: the mascot shows how you're doing; bars and conditions beside it.
fn update_status(
    real: Res<Time<Real>>,
    st: Res<PipState>,
    game: Res<Game>,
    weather: Res<WeatherRes>,
    clock: Res<ClockRes>,
    art: Res<MascotArt>,
    player: Query<&Transform, With<Player>>,
    mut mascot: Query<(&mut ImageNode, &mut Node), (With<StatusMascot>, Without<StatusGlow>)>,
    mut glow: Query<&mut ImageNode, (With<StatusGlow>, Without<StatusMascot>)>,
    mut bars: Query<(&mut Node, &StatusBar), (Without<StatusMascot>, Without<StatusGlow>)>,
    mut text: Query<&mut Text, (With<StatusText>, Without<StatusSide>)>,
    mut side: Query<&mut Text, (With<StatusSide>, Without<StatusText>)>,
) {
    if st.nav.page != Page::Status {
        return;
    }
    let t = real.elapsed_secs();
    let s = &game.survival;
    let hp = s.health / s.max_health().max(1.0);
    let cold = s.body_heat < 40.0;
    let hurt = hp < 0.35;
    let irradiated = s.rads > 100.0;
    if let Ok((mut img, mut node)) = mascot.single_mut() {
        img.image = if hurt { art.hurt.clone() } else if cold { art.cold.clone() } else { art.idle.clone() };
        // Idle: a cheerful bob. Cold: a fast shiver.
        let (dx, dy) = if cold { ((t * 47.0).sin() * 2.5, 0.0) } else { (0.0, (t * 3.0).sin().abs() * -4.0) };
        node.left = Val::Px(dx);
        node.top = Val::Px(dy);
    }
    if let Ok(mut g) = glow.single_mut() {
        let a = if irradiated { (0.35 + 0.3 * (t * 4.0).sin()) * (s.rads / 400.0).clamp(0.4, 1.0) } else { 0.0 };
        g.color = AMBER.with_alpha(a);
    }
    for (mut node, b) in &mut bars {
        let v = match b.0 {
            0 => hp,
            1 => s.body_heat / 100.0,
            _ => s.rads / Survival::MAX_RADS,
        };
        node.width = Val::Percent(v.clamp(0.0, 1.0) * 100.0);
    }
    let pos = player.single().map(|t| t.translation).unwrap_or(Vec3::ZERO);
    let sheltered = terrain::shelter_at(pos.x, pos.z).is_some();
    let cond = weather.weather.conditions();
    let air = cond.air_temp_f + clock.0.temp_offset_f();
    let feels = Survival::effective_temp(&Exposure { air_temp_f: air, wind_chill_f: cond.wind_chill_f, sheltered, ..Default::default() });
    let mut effects = Vec::new();
    if cold {
        effects.push("COLD  heat draining fast");
    }
    if s.frostbite {
        effects.push("FROSTBITE  -HP over time");
    }
    if irradiated {
        effects.push("IRRADIATED  max HP reduced");
    }
    if hurt {
        effects.push("BADLY HURT  use a Stimpak (H)");
    }
    if game.inv.has_frostfang_coat {
        effects.push("FROSTFANG COAT  +insulation");
    }
    if sheltered {
        effects.push("SHELTERED  warming up");
    }
    if effects.is_empty() {
        effects.push("None. Feeling Minnesota fine.");
    }
    let weather_line = match weather.weather.phase {
        Phase::Calm => "Calm".to_string(),
        Phase::Warning => format!("SIREN: blizzard in {:.0}s", weather.weather.timer.max(0.0)),
        Phase::Blizzard => format!("RAD-BLIZZARD {:.0}s", weather.weather.timer.max(0.0)),
    };
    if let Ok(mut tx) = text.single_mut() {
        tx.0 = format!(
            "CONDITIONS\n{}\n\nWEATHER\n  {}\n  Air {:.0}F, feels {:.0}F\n\nRADIATION\n  {:.0} rads\n  {}",
            effects.iter().map(|e| format!("  {e}")).collect::<Vec<_>>().join("\n"),
            weather_line,
            air,
            feels,
            s.rads,
            match Survival::BASE_MAX_HEALTH - s.max_health() {
                lost if lost >= 0.5 => format!("Max HP -{lost:.0}"),
                _ => "Max HP unaffected".to_string(),
            },
        );
    }
    if let Ok(mut tx) = side.single_mut() {
        tx.0 = format!(
            "THAWBORN\nVault 143\n\nHP    {:.0}/{:.0}\nHEAT  {:.0}%\nRADS  {:.0}\n\nKILLS {}\nPELTS {}\nSCRAP {}",
            s.health,
            s.max_health(),
            s.body_heat,
            s.rads,
            game.kills,
            game.inv.pelts,
            game.inv.scrap
        );
    }
}

/// The rows and details for a list page.
fn list_items(page: Page, game: &Game, slots: &SlotSummaries) -> Vec<(String, String)> {
    let inv = &game.inv;
    match page {
        Page::Special => [
            ("STRENGTH", 5, "Hauling hotdish and ice augers. Affects how hard you swing the ice axe."),
            ("PERCEPTION", 6, "Spotting a Frostfang against fresh snow, or a cache under a drift."),
            ("ENDURANCE", 6, "Mille Lacs winters are long. So is the Long Winter."),
            ("CHARISMA", 4, "Minnesota Nice. Mostly wasted on wolves."),
            ("INTELLIGENCE", 6, "Enough to fit a choke to a scrap shotgun at a bait shop bench."),
            ("AGILITY", 5, "How fast you get that magazine in with numb fingers."),
            ("LUCK", 5, "The vault opened. The bomb in the crater didn't. You're doing okay."),
        ]
        .into_iter()
        .map(|(n, v, d)| (format!("{n:<14}{v:>2}"), format!("{n}  {v}\n\n{d}")))
        .collect(),
        Page::Weapons => WeaponKind::ALL
            .iter()
            .filter(|k| game.arsenal.owned[k.slot()])
            .map(|&k| {
                let w = game.arsenal.get(k);
                let held = if game.arsenal.current == k.slot() { "*" } else { " " };
                let label = format!("{held}{}", k.name());
                let ammo = match k.ammo() {
                    Some(a) => format!("{}/{}  +{} {}", w.mag, w.mag_size, inv.reserve(a), a.name()),
                    None => "melee".to_string(),
                };
                let fitted: Vec<&str> = Upgrade::ALL.iter().filter(|u| w.has_upgrade(**u)).map(|u| u.name()).collect();
                let next = match w.next_upgrade() {
                    Some(up) if k.ammo().is_some() => format!("NEXT MOD  {} ({} scrap)\n  {}", up.name(), up.scrap_cost(), up.describe()),
                    _ => "Fully modded.".to_string(),
                };
                let detail = format!(
                    "{}\n\nDAM   {:.0}{}\nAMMO  {}\nRATE  {:.1}/s\nRANGE {:.0} m\nCND   {}\n\nMODS  {}\n\n{}",
                    k.name().to_uppercase(),
                    w.damage,
                    if w.pellets > 1 { format!(" x{}", w.pellets) } else { String::new() },
                    ammo,
                    1.0 / w.fire_interval.max(0.01),
                    w.range,
                    bar(if w.jammed { 0.4 } else { 0.9 }, 10),
                    if fitted.is_empty() { "none".to_string() } else { fitted.join(", ") },
                    next
                );
                (label, detail)
            })
            .collect(),
        Page::Apparel => {
            let mut v = vec![
                ("*Vault 143 Jumpsuit".to_string(), "VAULT 143 JUMPSUIT\n\nDT 1   WARMTH +5\n\nBlue and gold, freshly pressed in 2077.\nNot rated for minus forty.".to_string()),
                ("*Toque and Scarf".to_string(), "TOQUE AND SCARF\n\nDT 0   WARMTH +5\n\nKnitted by the Overseer's aunt.\nThe pom-pom is regulation.".to_string()),
                ("*Pip-Boy 3000".to_string(), "PIP-BOY 3000\n\nYour wrist computer: map, stats,\nGeiger counter. Battery: 197 years.".to_string()),
            ];
            if inv.has_frostfang_coat {
                v.insert(0, ("*Frostfang Coat".to_string(), "FROSTFANG COAT\n\nDT 3   WARMTH +40\n\nStitched from Frostfang pelts at a fish\nhouse workbench. Smells like wet wolf.".to_string()));
            } else {
                v.push((
                    " Frostfang Coat (not made)".to_string(),
                    format!("FROSTFANG COAT\n\nNot crafted yet: {} of {} pelts.\nPress C at a fish house.", inv.pelts, Inventory::PELTS_FOR_COAT),
                ));
            }
            v
        }
        Page::Aid => vec![
            (format!("Stimpak ({})", inv.stimpaks), "STIMPAK\n\n+40 HP\n\nENTER to use (or H any time).".to_string()),
            (format!("RadAway ({})", inv.radaway), "RADAWAY\n\n-150 rads\n\nENTER to use (or X any time).".to_string()),
            (format!("Vault 143 Hotdish ({})", inv.hotdish), "VAULT 143 HOTDISH\n\n+35 HEAT  +5 HP\n\nTater tots, cream of mushroom and\nsomething that glows. ENTER or F.".to_string()),
        ],
        Page::Saves => (0..SLOTS)
            .map(|slot| {
                let name = slot_name(slot);
                let summary = slots.0[slot].clone().unwrap_or_else(|| "- empty -".to_string());
                let how = match slot {
                    AUTOSAVE => "Written for you when you sleep or take\nshelter. L loads it.".to_string(),
                    QUICKSAVE => "F4 saves here from anywhere in the game.\nENTER saves now, L loads it.".to_string(),
                    _ => "ENTER saves here, L loads it.".to_string(),
                };
                (format!("{name:<10}"), format!("{}\n\n{}\n\n{}", name.to_uppercase(), summary, how))
            })
            .collect(),
        _ => Vec::new(),
    }
}

fn update_list(
    st: Res<PipState>,
    game: Res<Game>,
    slots: Res<SlotSummaries>,
    art: Res<MascotArt>,
    mut rows: Query<(&ListRow, &mut Node, &mut BorderColor, &mut BackgroundColor, &Children)>,
    mut texts: Query<&mut Text, Without<DetailText>>,
    mut detail: Query<&mut Text, With<DetailText>>,
    mut mascot: Query<(&mut ImageNode, &mut Node), (With<DetailMascot>, Without<ListRow>)>,
) {
    if st.nav.page.layout() != Layout::List {
        return;
    }
    let items = list_items(st.nav.page, &game, &slots);
    let sel = st.nav.selected[st.nav.page.index()].min(items.len().saturating_sub(1));
    for (row, mut node, mut border, mut bg, children) in &mut rows {
        let Some((label, _)) = items.get(row.0) else {
            node.display = Display::None;
            continue;
        };
        node.display = Display::Flex;
        let active = row.0 == sel;
        border.0 = if active { AMBER } else { Color::NONE };
        bg.0 = if active { AMBER_FAINT } else { Color::NONE };
        if let Some(&child) = children.first() {
            if let Ok(mut t) = texts.get_mut(child) {
                if &t.0 != label {
                    t.0 = label.clone();
                }
            }
        }
    }
    if let Ok(mut t) = detail.single_mut() {
        t.0 = items.get(sel).map(|i| i.1.clone()).unwrap_or_else(|| "Nothing here yet.".to_string());
    }
    if let Ok((mut img, mut node)) = mascot.single_mut() {
        let show = st.nav.page == Page::Special;
        node.display = if show { Display::Flex } else { Display::None };
        img.image = art.idle.clone();
    }
}

/// NOTES: objectives (ticked as you go), places found, and settings.
fn update_notes(st: Res<PipState>, game: Res<Game>, audio: Res<AudioSettings>, geiger: Res<GeigerOn>, mut q: Query<&mut Text, With<NotesText>>) {
    if st.nav.page != Page::Notes {
        return;
    }
    let Ok(mut t) = q.single_mut() else { return };
    let tick = |done: bool| if done { "[X]" } else { "[ ]" };
    let owned = game.arsenal.owned.iter().filter(|o| **o).count();
    let marks = mapdata::landmarks();
    let found: Vec<&str> = marks.iter().enumerate().filter(|(i, _)| st.fog.landmarks_found[*i]).map(|(_, l)| l.name).collect();
    let pct = |v: f32| (v * 100.0).round() as i32;
    t.0 = format!(
        "OBJECTIVES\n  {} Leave Vault 143\n  {} Find a better gun than the pipe rifle ({}/4 weapons)\n  {} Mod a weapon at a fish house workbench\n  {} Craft a Frostfang coat ({}/{} pelts)\n  {} Explore Mille Lacs ({}/{} places, {:.0}% of the map)\n\n\
         PLACES FOUND\n  {}\n\n\
         SETTINGS\n  Sound: master {}%  effects {}%  music {}%  ambience {}%{}\n  Geiger counter {}   (F10/F11, F5-F8, F9 mute, G Geiger)",
        tick(true),
        tick(owned > 1),
        owned,
        tick(total_upgrades(&game) > 0),
        tick(game.inv.has_frostfang_coat),
        game.inv.pelts.min(Inventory::PELTS_FOR_COAT),
        Inventory::PELTS_FOR_COAT,
        tick(found.len() == marks.len()),
        found.len(),
        marks.len(),
        st.fog.fraction() * 100.0,
        if found.is_empty() { "none yet".to_string() } else { found.join(", ") },
        pct(audio.0.master),
        pct(audio.0.sfx),
        pct(audio.0.music),
        pct(audio.0.ambience),
        if audio.0.muted { "  MUTED" } else { "" },
        if geiger.0 { "ON" } else { "OFF" },
    );
}

/// Place the map, markers, player arrow and cursor for the current zoom and centre.
fn update_map(
    st: Res<PipState>,
    player: Query<(&Transform, &Player)>,
    mut world: Query<&mut Node, (With<MapWorld>, Without<PlayerMarker>, Without<MapCursor>, Without<CursorLabel>)>,
    mut markers: Query<(&mut Visibility, &LandmarkMarker), (Without<LandmarkLabel>, Without<MapCursor>, Without<CursorLabel>)>,
    mut labels: Query<(&mut Visibility, &LandmarkLabel), (Without<LandmarkMarker>, Without<MapCursor>, Without<CursorLabel>)>,
    mut arrow: Query<(&mut Transform, &mut Node), (With<PlayerMarker>, Without<MapWorld>, Without<Player>, Without<MapCursor>, Without<CursorLabel>)>,
    view: Query<&RelativeCursorPosition, With<MapView>>,
    mut cursor: Query<(&mut Node, &mut Visibility, &ComputedNode), (With<MapCursor>, Without<MapWorld>, Without<PlayerMarker>, Without<LandmarkMarker>, Without<LandmarkLabel>, Without<CursorLabel>)>,
    mut cursor_label: Query<(&mut Text, &mut Node), (With<CursorLabel>, Without<MapWorld>, Without<PlayerMarker>, Without<MapCursor>, Without<InfoText>)>,
    mut info: Query<&mut Text, (With<InfoText>, Without<CursorLabel>)>,
    clock: Res<ClockRes>,
) {
    if st.nav.page != Page::Map {
        return;
    }
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
        let show = found[l.0] && (st.zoom >= 1.8 || mapdata::landmarks()[l.0].kind == Kind::Vault);
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

    // The cursor: crosshair lines through the mouse, and the place under it.
    let marks = mapdata::landmarks();
    let mut hovered = None;
    let rel = view.single().ok().and_then(|r| r.normalized).filter(|r| (0.0..=1.0).contains(&r.x) && (0.0..=1.0).contains(&r.y));
    for (mut n, mut vis, computed) in &mut cursor {
        let Some(r) = rel else {
            *vis = Visibility::Hidden;
            continue;
        };
        *vis = Visibility::Inherited;
        let horizontal = computed.size().x > computed.size().y;
        if horizontal {
            n.top = Val::Px(r.y * VIEW);
        } else {
            n.left = Val::Px(r.x * VIEW);
        }
    }
    let world_at = rel.map(|r| {
        let (cu, cv) = mapdata::to_uv(st.center.x, st.center.y);
        let mu = cu + (r.x - 0.5) / st.zoom;
        let mv = cv + (r.y - 0.5) / st.zoom;
        Vec2::new((mu - 0.5) * mapdata::WORLD, (mv - 0.5) * mapdata::WORLD)
    });
    if let Some(w) = world_at {
        let pick = 12.0 / st.zoom + 4.0;
        hovered = marks.iter().enumerate().filter(|(i, l)| found[*i] && (l.x - w.x).hypot(l.z - w.y) < pick).map(|(_, l)| l).next();
    }
    if let Ok((mut t, mut n)) = cursor_label.single_mut() {
        match (rel, world_at) {
            (Some(r), Some(w)) => {
                t.0 = match hovered {
                    Some(l) => l.name.to_string(),
                    None => format!("{:.0}, {:.0}", w.x, w.y),
                };
                n.left = Val::Px((r.x * VIEW + 8.0).min(VIEW - 120.0));
                n.top = Val::Px((r.y * VIEW + 6.0).min(VIEW - 16.0));
            }
            _ => t.0.clear(),
        }
    }
    if let Ok(mut t) = info.single_mut() {
        let nearest = marks
            .iter()
            .enumerate()
            .filter(|(i, _)| found[*i])
            .map(|(_, l)| (l, (l.x - ptf.translation.x).hypot(l.z - ptf.translation.z)))
            .min_by(|a, b| a.1.total_cmp(&b.1));
        let (cx, cz) = (ptf.translation.x, ptf.translation.z);
        let heading = ((-p.yaw).to_degrees().rem_euclid(360.0).round() as i32) % 360;
        let places = found.iter().filter(|f| **f).count();
        let mut s = format!("MILLE LACS COUNTY\nDAY {}  {}\n\nYOU  {:.0}, {:.0}\nHEADING {:03}\n\n", clock.0.day, clock.0.label(), cx, cz, heading);
        if let Some((l, d)) = nearest {
            s += &format!("NEAREST\n  {}\n  {:.0} m\n\n", l.name, d);
        }
        s += &format!("EXPLORED  {:.0}%\nPLACES  {}/{}\n\n", st.fog.fraction() * 100.0, places, marks.len());
        s += "<> filled  Vault 143\n<> hollow  found places\n\nWheel zoom, drag/WASD pan,\nC centre on you";
        t.0 = s;
    }
}
