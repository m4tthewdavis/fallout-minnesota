//! The pause menu (Esc): Resume, Save, Load, Settings and Quit, in the same
//! amber CRT style as the Pip-Boy. The rules (screens, rows, what each choice
//! does) are in `sim::menu` and `sim::settings`; this file draws them, applies
//! the settings to the running game (shadows, view distance, UI size, volume)
//! and keeps them in `settings.json`.

use bevy::pbr::{CascadeShadowConfigBuilder, DirectionalLightShadowMap};
use bevy::prelude::*;
use bevy::ui::widget::NodeImageMode;
use bevy::ui::{FocusPolicy, RelativeCursorPosition};
use bevy::window::PrimaryWindow;

use crate::assets::GameAssets;
use crate::audio::AudioSettings;
use crate::player::{set_grab, Player};
use crate::sim::menu::{Action, Context, Menu, RowKind, SLOTS};
use crate::sim::settings::Settings;
use crate::state::{Game, Messages, Paused, PipOpen};
use crate::storage;
use crate::world::{PointShadows, Sun};

const AMBER: Color = Color::srgb(1.0, 0.72, 0.3);
const AMBER_DIM: Color = Color::srgba(1.0, 0.72, 0.3, 0.4);
const AMBER_FAINT: Color = Color::srgba(1.0, 0.72, 0.3, 0.16);
const AMBER_OFF: Color = Color::srgba(1.0, 0.72, 0.3, 0.28);
const PANEL_BG: Color = Color::srgba(0.075, 0.042, 0.012, 0.96);
/// Most rows any screen has (the settings screen).
const MAX_ROWS: usize = 8;

const SETTINGS_FILE: &str = "settings.json";

/// The player's settings, as applied to the running game.
#[derive(Resource)]
pub struct GameSettings(pub Settings);

/// One line describing each save slot, or `None` if it's empty. The save
/// system keeps this up to date.
#[derive(Resource, Default)]
pub struct SlotSummaries(pub [Option<String>; SLOTS]);

/// Something the menu asked for that the save system carries out.
#[derive(Event, Clone, Copy, Debug)]
pub enum SaveRequest {
    Save(usize),
    Load(usize),
}

#[derive(Resource, Default)]
struct MenuState {
    menu: Menu,
    /// Settings changed and should be written to disk soon.
    persist_since: Option<f32>,
}

#[derive(Component)]
struct MenuRoot;
#[derive(Component)]
struct MenuTitle;
#[derive(Component)]
struct MenuNote;
#[derive(Component)]
struct MenuRow(usize);
#[derive(Component)]
struct RowLabel(usize);
#[derive(Component)]
struct RowValue(usize);

pub struct MenuPlugin;

impl Plugin for MenuPlugin {
    fn build(&self, app: &mut App) {
        // Screenshots shouldn't depend on (or overwrite) the player's own settings.
        let settings = if std::env::var("FMN_SHOT").is_ok() {
            // FMN_SHADOWS=off|low|medium|high picks the quality (software
            // rendering draws shadow maps slowly, so tests often turn them off).
            use crate::sim::settings::ShadowQuality;
            let shadows = match std::env::var("FMN_SHADOWS").as_deref() {
                Ok("off") => ShadowQuality::Off,
                Ok("low") => ShadowQuality::Low,
                Ok("medium") => ShadowQuality::Medium,
                _ => ShadowQuality::High,
            };
            Settings { shadows, ..Settings::default() }
        } else {
            storage::read(&storage::data_dir(), SETTINGS_FILE).and_then(|t| Settings::from_json(&t)).unwrap_or_default()
        };
        app.insert_resource(GameSettings(settings))
            .init_resource::<MenuState>()
            .init_resource::<SlotSummaries>()
            .add_event::<SaveRequest>()
            .add_systems(Startup, build_menu)
            .add_systems(
                Update,
                (
                    auto_open,
                    menu_system,
                    render_menu,
                    apply_settings.run_if(resource_changed::<GameSettings>),
                    sync_audio_back,
                    persist_settings,
                )
                    .chain(),
            );
    }
}

fn build_menu(mut commands: Commands, assets: Res<GameAssets>) {
    let font = |size: f32| TextFont { font: assets.font.clone(), font_size: size, ..default() };
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.62)),
            GlobalZIndex(150),
            Visibility::Hidden,
            MenuRoot,
        ))
        .with_children(|root| {
            root.spawn((
                Node {
                    width: Val::Px(640.0),
                    flex_direction: FlexDirection::Column,
                    padding: UiRect::axes(Val::Px(34.0), Val::Px(26.0)),
                    border: UiRect::all(Val::Px(3.0)),
                    row_gap: Val::Px(4.0),
                    overflow: Overflow::clip(),
                    ..default()
                },
                BackgroundColor(PANEL_BG),
                BorderColor(AMBER),
                BorderRadius::all(Val::Px(14.0)),
            ))
            .with_children(|p| {
                p.spawn((
                    Node { border: UiRect::bottom(Val::Px(2.0)), padding: UiRect::bottom(Val::Px(6.0)), margin: UiRect::bottom(Val::Px(6.0)), ..default() },
                    BorderColor(AMBER),
                ))
                .with_children(|h| {
                    h.spawn((Text::new("PAUSED"), font(32.0), TextColor(AMBER), MenuTitle));
                });
                p.spawn((Text::new(""), font(15.0), TextColor(AMBER_DIM), MenuNote));
                for i in 0..MAX_ROWS {
                    p.spawn((
                        Button,
                        Node {
                            width: Val::Percent(100.0),
                            justify_content: JustifyContent::SpaceBetween,
                            align_items: AlignItems::Center,
                            padding: UiRect::axes(Val::Px(14.0), Val::Px(6.0)),
                            border: UiRect::all(Val::Px(1.0)),
                            ..default()
                        },
                        BorderColor(Color::NONE),
                        BackgroundColor(Color::NONE),
                        RelativeCursorPosition::default(),
                        MenuRow(i),
                    ))
                    .with_children(|r| {
                        r.spawn((Text::new(""), font(22.0), TextColor(AMBER), RowLabel(i)));
                        r.spawn((Text::new(""), font(18.0), TextColor(AMBER), RowValue(i)));
                    });
                }
                p.spawn((
                    Text::new("W S select    ENTER choose    A D change    ESC back"),
                    font(13.0),
                    TextColor(AMBER_DIM),
                    Node { margin: UiRect::top(Val::Px(12.0)), ..default() },
                ));
                // Scanlines over the panel.
                p.spawn((
                    Node { position_type: PositionType::Absolute, left: Val::Px(0.0), top: Val::Px(0.0), width: Val::Percent(100.0), height: Val::Percent(100.0), ..default() },
                    ImageNode {
                        image: assets.scanlines.clone(),
                        color: Color::srgba(0.0, 0.0, 0.0, 0.8),
                        image_mode: NodeImageMode::Tiled { tile_x: true, tile_y: true, stretch_value: 1.0 },
                        ..default()
                    },
                    FocusPolicy::Pass,
                ));
            });
        });
}

fn context(game: &Game, slots: &SlotSummaries) -> Context {
    Context { alive: game.death.is_none(), slots: slots.0.clone() }
}

/// Screenshot mode: FMN_MENU=main|settings|save|load|quit opens the menu on
/// that screen after a moment.
fn auto_open(real: Res<Time<Real>>, mut stage: Local<u8>, mut keys: ResMut<ButtonInput<KeyCode>>, mut state: ResMut<MenuState>) {
    let Ok(which) = std::env::var("FMN_MENU") else { return };
    let t = real.elapsed_secs();
    if *stage == 0 && t > 1.0 {
        *stage = 1;
        keys.press(KeyCode::Escape);
    } else if *stage == 1 && t > 1.6 {
        *stage = 2;
        use crate::sim::menu::Screen;
        let screen = match which.as_str() {
            "settings" => Screen::Settings,
            "save" => Screen::Save,
            "load" => Screen::Load,
            "quit" => Screen::ConfirmQuit,
            _ => Screen::Main,
        };
        state.menu.screen = screen;
        state.menu.selected = std::env::var("FMN_MENU_ROW").ok().and_then(|r| r.parse().ok()).unwrap_or(0);
    }
}

/// Esc opens and closes the menu; while it's open, the keyboard and mouse
/// drive it.
#[allow(clippy::too_many_arguments)]
fn menu_system(
    time: Res<Time<Real>>,
    keys: Res<ButtonInput<KeyCode>>,
    pip: Res<PipOpen>,
    talking: Res<crate::state::Talking>,
    game: Res<Game>,
    slots: Res<SlotSummaries>,
    mut paused: ResMut<Paused>,
    mut state: ResMut<MenuState>,
    mut settings: ResMut<GameSettings>,
    mut vtime: ResMut<Time<Virtual>>,
    mut windows: Query<&mut Window, With<PrimaryWindow>>,
    mut requests: EventWriter<SaveRequest>,
    mut exit: EventWriter<AppExit>,
    rows: Query<(&MenuRow, &Interaction, &RelativeCursorPosition), Changed<Interaction>>,
    mut pip_was_open: Local<bool>,
    mut talk_was_open: Local<bool>,
) {
    let esc = keys.just_pressed(KeyCode::Escape);
    // Esc that just closed the Pip-Boy must not also open the menu.
    // The same goes for Esc that just left a conversation.
    let pip_busy = pip.0 || *pip_was_open || talking.0 || *talk_was_open;
    *pip_was_open = pip.0;
    *talk_was_open = talking.0;
    if !paused.0 {
        if esc && !pip_busy {
            info!("menu: opened");
            paused.0 = true;
            state.menu = Menu::default();
            vtime.pause();
            if let Ok(mut w) = windows.single_mut() {
                set_grab(&mut w, false);
            }
        }
        return;
    }

    let ctx = context(&game, &slots);
    let mut action = Action::None;
    let mut changed = false;
    let current = state.menu.rows(&settings.0, &ctx);
    let before = state.menu.clone();

    if esc {
        action = state.menu.back();
    }
    let up = keys.just_pressed(KeyCode::ArrowUp) || keys.just_pressed(KeyCode::KeyW);
    let down = keys.just_pressed(KeyCode::ArrowDown) || keys.just_pressed(KeyCode::KeyS);
    if up {
        state.menu.move_selection(-1, &current);
    }
    if down {
        state.menu.move_selection(1, &current);
    }
    let left = keys.just_pressed(KeyCode::ArrowLeft) || keys.just_pressed(KeyCode::KeyA);
    let right = keys.just_pressed(KeyCode::ArrowRight) || keys.just_pressed(KeyCode::KeyD);
    if left {
        changed |= state.menu.adjust(-1, &mut settings.bypass_change_detection().0);
    }
    if right {
        changed |= state.menu.adjust(1, &mut settings.bypass_change_detection().0);
    }
    if keys.just_pressed(KeyCode::Enter) || keys.just_pressed(KeyCode::NumpadEnter) || keys.just_pressed(KeyCode::Space) {
        let (a, c) = state.menu.activate(&mut settings.bypass_change_detection().0, &ctx);
        action = if a == Action::None { action } else { a };
        changed |= c;
    }
    // The mouse: hovering selects; clicking a button presses it, clicking a
    // setting nudges it left or right depending on which half you hit.
    for (row, interaction, cursor) in &rows {
        match interaction {
            Interaction::Hovered => {
                state.menu.select_row(row.0, &current);
            }
            Interaction::Pressed => {
                state.menu.select_row(row.0, &current);
                let Some(r) = current.get(row.0) else { continue };
                if !r.enabled {
                    continue;
                }
                match r.kind {
                    RowKind::Button => {
                        let (a, c) = state.menu.activate(&mut settings.bypass_change_detection().0, &ctx);
                        action = if a == Action::None { action } else { a };
                        changed |= c;
                    }
                    RowKind::Choice | RowKind::Slider => {
                        let dir = if cursor.normalized.is_some_and(|n| n.x < 0.5) { -1 } else { 1 };
                        changed |= state.menu.adjust(dir, &mut settings.bypass_change_detection().0);
                    }
                }
            }
            Interaction::None => {}
        }
    }
    if changed {
        settings.set_changed();
        state.persist_since = Some(time.elapsed_secs());
    }
    if state.menu != before {
        info!("menu: {:?}", state.menu);
    }

    match action {
        Action::None => {}
        Action::Resume => {
            close_menu(&mut paused, &mut vtime, &mut windows);
            // Leaving the menu: write any changed settings now.
            if state.persist_since.is_some() {
                state.persist_since = Some(f32::NEG_INFINITY);
            }
        }
        Action::Save(slot) => {
            requests.write(SaveRequest::Save(slot));
        }
        Action::Load(slot) => {
            requests.write(SaveRequest::Load(slot));
        }
        Action::Quit => {
            state.persist_since = Some(f32::NEG_INFINITY);
            exit.write(AppExit::Success);
        }
    }
}

/// Draw the current screen into the pooled rows.
#[allow(clippy::type_complexity)]
fn render_menu(
    paused: Res<Paused>,
    state: Res<MenuState>,
    settings: Res<GameSettings>,
    slots: Res<SlotSummaries>,
    game: Res<Game>,
    mut root: Query<&mut Visibility, With<MenuRoot>>,
    mut title: Query<&mut Text, (With<MenuTitle>, Without<MenuNote>, Without<RowLabel>, Without<RowValue>)>,
    mut note: Query<&mut Text, (With<MenuNote>, Without<MenuTitle>, Without<RowLabel>, Without<RowValue>)>,
    mut rows: Query<(&MenuRow, &mut Node, &mut BorderColor, &mut BackgroundColor)>,
    mut labels: Query<(&RowLabel, &mut Text, &mut TextColor), (Without<MenuTitle>, Without<MenuNote>, Without<RowValue>)>,
    mut values: Query<(&RowValue, &mut Text, &mut TextColor), (Without<MenuTitle>, Without<MenuNote>, Without<RowLabel>)>,
) {
    let Ok(mut vis) = root.single_mut() else { return };
    let want = if paused.0 { Visibility::Visible } else { Visibility::Hidden };
    vis.set_if_neq(want);
    if !paused.0 {
        return;
    }
    let ctx = context(&game, &slots);
    let menu = &state.menu;
    let list = menu.rows(&settings.0, &ctx);
    if let Ok(mut t) = title.single_mut() {
        t.set_if_neq(Text::new(menu.title()));
    }
    if let Ok(mut t) = note.single_mut() {
        t.set_if_neq(Text::new(menu.note()));
    }
    for (row, mut node, mut border, mut bg) in &mut rows {
        let display = if row.0 < list.len() { Display::Flex } else { Display::None };
        if node.display != display {
            node.display = display;
        }
        let active = row.0 == menu.selected;
        border.set_if_neq(BorderColor(if active { AMBER } else { Color::NONE }));
        bg.set_if_neq(BackgroundColor(if active { AMBER_FAINT } else { Color::NONE }));
    }
    for (l, mut text, mut color) in &mut labels {
        let Some(r) = list.get(l.0) else { continue };
        text.set_if_neq(Text::new(r.label.clone()));
        color.set_if_neq(TextColor(if r.enabled { AMBER } else { AMBER_OFF }));
    }
    for (v, mut text, mut color) in &mut values {
        let Some(r) = list.get(v.0) else { continue };
        text.set_if_neq(Text::new(r.value.clone()));
        color.set_if_neq(TextColor(if r.enabled { AMBER } else { AMBER_OFF }));
    }
}

/// Make the running game match the settings.
#[allow(clippy::too_many_arguments)]
fn apply_settings(
    mut commands: Commands,
    settings: Res<GameSettings>,
    mut audio: ResMut<AudioSettings>,
    mut shadow_map: ResMut<DirectionalLightShadowMap>,
    mut suns: Query<(Entity, &mut DirectionalLight), With<Sun>>,
    mut point_lights: Query<&mut PointLight, With<PointShadows>>,
    mut cameras: Query<&mut Projection, With<Player>>,
) {
    let s = &settings.0;
    info!("settings applied: {s:?}");
    // Sound.
    let mix = &mut audio.0;
    if (mix.master, mix.sfx, mix.music, mix.ambience) != (s.master, s.sfx, s.music, s.ambience) {
        (mix.master, mix.sfx, mix.music, mix.ambience) = (s.master, s.sfx, s.music, s.ambience);
    }
    // Shadows.
    let q = s.shadows;
    if shadow_map.size != q.map_size() {
        shadow_map.size = q.map_size();
    }
    for (sun, mut light) in &mut suns {
        light.shadows_enabled = q.sun_shadows();
        commands.entity(sun).insert(
            CascadeShadowConfigBuilder {
                num_cascades: q.cascades(),
                first_cascade_far_bound: (q.shadow_distance() * 0.1).max(4.0),
                maximum_distance: q.shadow_distance().max(8.0),
                ..default()
            }
            .build(),
        );
    }
    for mut light in &mut point_lights {
        light.shadows_enabled = q.point_light_shadows();
    }
    // View distance.
    for mut proj in &mut cameras {
        if let Projection::Perspective(p) = proj.as_mut() {
            p.far = s.view.far_plane();
        }
    }
}

/// F5-F11 change the volume directly; keep the saved settings in step.
fn sync_audio_back(audio: Res<AudioSettings>, mut settings: ResMut<GameSettings>, mut state: ResMut<MenuState>, time: Res<Time<Real>>) {
    if !audio.is_changed() {
        return;
    }
    let m = &audio.0;
    let s = &settings.0;
    if (s.master, s.sfx, s.music, s.ambience) != (m.master, m.sfx, m.music, m.ambience) {
        let s = &mut settings.bypass_change_detection().0;
        (s.master, s.sfx, s.music, s.ambience) = (m.master, m.sfx, m.music, m.ambience);
        state.persist_since = Some(time.elapsed_secs());
    }
}

/// Write the settings to disk a moment after the last change (or at once
/// when the menu closes or the game quits).
fn persist_settings(time: Res<Time<Real>>, settings: Res<GameSettings>, mut state: ResMut<MenuState>, mut msgs: ResMut<Messages>) {
    let Some(since) = state.persist_since else { return };
    if time.elapsed_secs() - since < 1.0 {
        return;
    }
    state.persist_since = None;
    if std::env::var("FMN_SHOT").is_ok() {
        return;
    }
    if let Err(e) = storage::write(&storage::data_dir(), SETTINGS_FILE, &settings.0.to_json()) {
        warn!("could not save settings: {e}");
        msgs.show("Could not save your settings.", 3.0);
    }
}

/// Close the pause menu and give the game back to the player.
pub fn close_menu(paused: &mut Paused, vtime: &mut Time<Virtual>, windows: &mut Query<&mut Window, With<PrimaryWindow>>) {
    info!("menu: closed");
    paused.0 = false;
    vtime.unpause();
    if let Ok(mut w) = windows.single_mut() {
        set_grab(&mut w, true);
    }
}
