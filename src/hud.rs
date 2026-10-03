//! Pip-Boy green HUD: icon bars for health (with the red slice radiation has
//! eaten), Body Heat and rads, a compass with the vault and the nearest
//! shelter marked, weapon and inventory readouts, messages, crosshair, and
//! CRT scanlines, vignette, frost, radiation and damage overlays.

use bevy::prelude::*;
use bevy::ui::widget::NodeImageMode;

use crate::assets::{GameAssets, MissingAssets};
use crate::player::Player;
use crate::sim::survival::{Exposure, Inventory, Survival};
use crate::sim::terrain::{self, SHELTERS, VAULT_POS};
use crate::sim::weather::Phase;
use crate::state::{ClockRes, Game, Messages, WeatherRes};

const PIP_GREEN: Color = Color::srgb(0.45, 1.0, 0.45);
const PIP_DIM: Color = Color::srgba(0.45, 1.0, 0.45, 0.35);
const PANEL: Color = Color::srgba(0.0, 0.07, 0.02, 0.55);
const WARN: Color = Color::srgb(1.0, 0.75, 0.3);
const DANGER: Color = Color::srgb(1.0, 0.3, 0.25);

#[derive(Clone, Copy, PartialEq, Eq)]
enum Stat {
    Hp,
    Heat,
    Rads,
}

#[derive(Component)]
struct BarFill(Stat);
#[derive(Component)]
struct ValueText(Stat);
/// The red end of the HP bar: max health lost to radiation.
#[derive(Component)]
struct RadLoss;
#[derive(Component)]
struct InfoText;
#[derive(Component)]
struct WeaponText;
#[derive(Component)]
struct AmmoText;
#[derive(Component)]
struct MessageText;
#[derive(Component)]
struct DeathText;
#[derive(Component)]
struct CompassText;
#[derive(Component)]
struct HeadingText;
#[derive(Component)]
struct HurtOverlay;
#[derive(Component)]
struct FrostOverlay;
#[derive(Component)]
struct RadOverlay;
#[derive(Component)]
struct HelpText;
#[derive(Component)]
struct MissingBanner;

pub struct HudPlugin;

impl Plugin for HudPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_hud).add_systems(Update, (update_hud, missing_banner));
    }
}

fn full_screen() -> Node {
    Node {
        position_type: PositionType::Absolute,
        width: Val::Percent(100.0),
        height: Val::Percent(100.0),
        justify_content: JustifyContent::Center,
        align_items: AlignItems::Center,
        ..default()
    }
}

fn panel(node: Node) -> (Node, BackgroundColor, BorderColor, BorderRadius) {
    (
        Node {
            border: UiRect::all(Val::Px(1.0)),
            padding: UiRect::axes(Val::Px(10.0), Val::Px(8.0)),
            ..node
        },
        BackgroundColor(PANEL),
        BorderColor(PIP_DIM),
        BorderRadius::all(Val::Px(3.0)),
    )
}

fn spawn_hud(mut commands: Commands, assets: Res<GameAssets>) {
    let font = |size: f32| TextFont {
        font: assets.font.clone(),
        font_size: size,
        ..default()
    };
    let image = |handle: &Handle<Image>, color: Color, mode: NodeImageMode| ImageNode {
        image: handle.clone(),
        color,
        image_mode: mode,
        ..default()
    };

    // Overlays first so text draws on top of them.
    commands.spawn((full_screen(), image(&assets.vignette, Color::WHITE, NodeImageMode::Stretch)));
    commands.spawn((full_screen(), BackgroundColor(Color::srgba(0.2, 1.0, 0.2, 0.0)), RadOverlay));
    commands.spawn((
        full_screen(),
        image(&assets.frost, Color::srgba(1.0, 1.0, 1.0, 0.0), NodeImageMode::Stretch),
        FrostOverlay,
    ));
    commands.spawn((full_screen(), BackgroundColor(Color::srgba(0.8, 0.0, 0.0, 0.0)), HurtOverlay));
    commands.spawn((
        full_screen(),
        image(
            &assets.scanlines,
            Color::srgba(1.0, 1.0, 1.0, 0.35),
            NodeImageMode::Tiled {
                tile_x: true,
                tile_y: true,
                stretch_value: 1.0,
            },
        ),
    ));

    // Crosshair.
    commands.spawn(full_screen()).with_children(|p| {
        p.spawn((Text::new("+"), font(24.0), TextColor(PIP_GREEN.with_alpha(0.85))));
    });

    // Controls reminder (fades after the first minute).
    commands.spawn((
        Text::new(
            "WASD move  SHIFT sprint  SPACE jump  LMB fire  R reload/unjam\n\
             H stimpak  X RadAway  F hotdish  C craft coat (at shelter)  ESC free mouse",
        ),
        font(13.0),
        TextColor(PIP_GREEN.with_alpha(0.6)),
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(12.0),
            top: Val::Px(10.0),
            ..default()
        },
        HelpText,
    ));

    // Compass along the top.
    commands
        .spawn(Node {
            position_type: PositionType::Absolute,
            width: Val::Percent(100.0),
            top: Val::Px(8.0),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            ..default()
        })
        .with_children(|p| {
            p.spawn(panel(Node {
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                padding: UiRect::axes(Val::Px(10.0), Val::Px(2.0)),
                ..default()
            }))
            .with_children(|c| {
                c.spawn((Text::new(""), font(16.0), TextColor(PIP_GREEN), CompassText));
                c.spawn((Text::new(""), font(12.0), TextColor(PIP_GREEN.with_alpha(0.7)), HeadingText));
            });
            p.spawn((
                Text::new(""),
                font(19.0),
                TextColor(WARN),
                TextLayout::new_with_justify(JustifyText::Center),
                Node {
                    margin: UiRect::top(Val::Px(14.0)),
                    max_width: Val::Px(900.0),
                    ..default()
                },
                MessageText,
            ));
        });

    // Status panel (bottom-left): three icon bars plus conditions.
    commands
        .spawn(panel(Node {
            position_type: PositionType::Absolute,
            left: Val::Px(14.0),
            bottom: Val::Px(14.0),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(5.0),
            ..default()
        }))
        .with_children(|p| {
            for (stat, label, icon) in [
                (Stat::Hp, "HP  ", &assets.icon_hp),
                (Stat::Heat, "HEAT", &assets.icon_heat),
                (Stat::Rads, "RADS", &assets.icon_rads),
            ] {
                p.spawn(Node {
                    align_items: AlignItems::Center,
                    column_gap: Val::Px(8.0),
                    ..default()
                })
                .with_children(|row| {
                    row.spawn((
                        image(icon, PIP_GREEN, NodeImageMode::Stretch),
                        Node {
                            width: Val::Px(20.0),
                            height: Val::Px(20.0),
                            ..default()
                        },
                    ));
                    row.spawn((Text::new(label), font(15.0), TextColor(PIP_GREEN)));
                    row.spawn((
                        Node {
                            width: Val::Px(190.0),
                            height: Val::Px(11.0),
                            border: UiRect::all(Val::Px(1.0)),
                            padding: UiRect::all(Val::Px(1.0)),
                            ..default()
                        },
                        BorderColor(PIP_DIM),
                    ))
                    .with_children(|bar| {
                        bar.spawn((
                            Node {
                                width: Val::Percent(100.0),
                                height: Val::Percent(100.0),
                                ..default()
                            },
                            BackgroundColor(PIP_GREEN),
                            BarFill(stat),
                        ));
                        if stat == Stat::Hp {
                            bar.spawn((
                                Node {
                                    position_type: PositionType::Absolute,
                                    right: Val::Px(1.0),
                                    top: Val::Px(1.0),
                                    bottom: Val::Px(1.0),
                                    width: Val::Percent(0.0),
                                    ..default()
                                },
                                BackgroundColor(DANGER.with_alpha(0.8)),
                                RadLoss,
                            ));
                        }
                    });
                    row.spawn((
                        Text::new(""),
                        font(15.0),
                        TextColor(PIP_GREEN),
                        Node {
                            min_width: Val::Px(120.0),
                            ..default()
                        },
                        ValueText(stat),
                    ));
                });
            }
            p.spawn((Text::new(""), font(14.0), TextColor(PIP_GREEN.with_alpha(0.85)), InfoText));
        });

    // Weapon panel (bottom-right).
    commands
        .spawn(panel(Node {
            position_type: PositionType::Absolute,
            right: Val::Px(14.0),
            bottom: Val::Px(14.0),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::FlexEnd,
            ..default()
        }))
        .with_children(|p| {
            p.spawn((Text::new(""), font(30.0), TextColor(PIP_GREEN), AmmoText));
            p.spawn((
                Text::new(""),
                font(14.0),
                TextColor(PIP_GREEN),
                TextLayout::new_with_justify(JustifyText::Right),
                WeaponText,
            ));
        });

    // Warning shown only if game files are missing.
    commands
        .spawn(Node {
            position_type: PositionType::Absolute,
            width: Val::Percent(100.0),
            top: Val::Percent(30.0),
            justify_content: JustifyContent::Center,
            ..default()
        })
        .with_children(|p| {
            p.spawn((
                Text::new(""),
                font(20.0),
                TextColor(DANGER),
                TextLayout::new_with_justify(JustifyText::Center),
                BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.0)),
                Node {
                    max_width: Val::Px(1000.0),
                    padding: UiRect::all(Val::Px(12.0)),
                    ..default()
                },
                MissingBanner,
            ));
        });

    // Death screen text.
    commands.spawn(full_screen()).with_children(|p| {
        p.spawn((
            Text::new(""),
            font(30.0),
            TextColor(DANGER),
            TextLayout::new_with_justify(JustifyText::Center),
            DeathText,
        ));
    });
}

fn missing_banner(
    time: Res<Time>,
    missing: Res<MissingAssets>,
    mut q: Query<(&mut Text, &mut BackgroundColor), With<MissingBanner>>,
) {
    let Ok((mut text, mut bg)) = q.single_mut() else { return };
    // Leave it up for the first 20 seconds, then shrink to a reminder.
    if missing.count == 0 {
        return;
    }
    text.0 = if time.elapsed_secs() < 20.0 {
        format!(
            "MISSING GAME FILES ({} could not be loaded, e.g. {})\n\n\
             The game looked for its 'assets' folder here:\n{}\n\n\
             Unzip the WHOLE download first (don't run the game from inside the zip),\n\
             and keep the 'assets' folder next to FalloutMinnesota.exe.",
            missing.count,
            missing.first.as_deref().unwrap_or("?"),
            missing.root,
        )
    } else {
        format!("{} game files missing - see the 'assets' folder note in README.md", missing.count)
    };
    bg.0 = Color::srgba(0.0, 0.0, 0.0, 0.7);
}

/// Compass bearing from north (-Z), clockwise, in degrees 0..360.
fn bearing(dx: f32, dz: f32) -> f32 {
    dx.atan2(-dz).to_degrees().rem_euclid(360.0)
}

/// A strip of compass ticks centred on `heading`, 2 degrees per character,
/// with markers for points of interest.
fn compass_strip(heading: f32, markers: &[(f32, char)]) -> String {
    const HALF: i32 = 30;
    let mut out = String::with_capacity(64);
    for i in -HALF..=HALF {
        let a = (heading + i as f32 * 2.0).rem_euclid(360.0);
        let near = |target: f32| {
            let d = (a - target + 540.0).rem_euclid(360.0) - 180.0;
            d.abs() < 1.0
        };
        let mut c = if near(0.0) {
            'N'
        } else if near(90.0) {
            'E'
        } else if near(180.0) {
            'S'
        } else if near(270.0) {
            'W'
        } else if [45.0, 135.0, 225.0, 315.0].iter().any(|&t| near(t)) {
            '+'
        } else if (a.round() as i32) % 10 == 0 {
            '|'
        } else {
            '.'
        };
        for &(target, m) in markers {
            if near(target) {
                c = m;
            }
        }
        if i == 0 && !"NESWVH".contains(c) {
            c = '^';
        }
        out.push(c);
    }
    out
}

#[allow(clippy::type_complexity, clippy::too_many_arguments)]
fn update_hud(
    time: Res<Time>,
    game: Res<Game>,
    weather: Res<WeatherRes>,
    clock: Res<ClockRes>,
    msgs: Res<Messages>,
    player: Query<(&Transform, &Player)>,
    mut bars: Query<(&mut Node, &mut BackgroundColor, &BarFill), Without<RadLoss>>,
    mut rad_loss: Query<&mut Node, (With<RadLoss>, Without<BarFill>)>,
    mut texts: ParamSet<(
        Query<(&mut Text, &mut TextColor, &ValueText)>,
        Query<&mut Text, With<InfoText>>,
        Query<&mut Text, With<WeaponText>>,
        Query<(&mut Text, &mut TextColor), With<AmmoText>>,
        Query<(&mut Text, &mut TextColor), With<MessageText>>,
        Query<&mut Text, With<DeathText>>,
        Query<&mut Text, With<CompassText>>,
        Query<&mut Text, With<HeadingText>>,
    )>,
    mut help: Query<&mut TextColor, (With<HelpText>, Without<ValueText>, Without<AmmoText>, Without<MessageText>)>,
    mut overlays: ParamSet<(
        Query<&mut BackgroundColor, (With<HurtOverlay>, Without<BarFill>)>,
        Query<&mut ImageNode, With<FrostOverlay>>,
        Query<&mut BackgroundColor, (With<RadOverlay>, Without<BarFill>)>,
    )>,
) {
    let s = &game.survival;
    let w = &weather.weather;
    let cond = w.conditions();
    let air_temp = cond.air_temp_f + clock.0.temp_offset_f();
    let t = time.elapsed_secs();

    let (pos, yaw) = player
        .single()
        .map(|(tf, p)| (tf.translation, p.yaw))
        .unwrap_or((Vec3::ZERO, 0.0));
    let sheltered = terrain::shelter_at(pos.x, pos.z).is_some();
    let ambient_rads = terrain::ambient_rads(pos.x, pos.z);
    let rad_rate = ambient_rads + if sheltered { 0.0 } else { cond.rads_per_sec };

    // ---- Bars ----
    for (mut node, mut bg, fill) in &mut bars {
        let (frac, color) = match fill.0 {
            Stat::Hp => {
                let f = s.health / Survival::BASE_MAX_HEALTH;
                (f, if f < 0.25 { DANGER } else { PIP_GREEN })
            }
            Stat::Heat => {
                let f = s.body_heat / 100.0;
                let c = if s.frostbite {
                    DANGER
                } else if f < 0.3 {
                    Color::srgb(0.55, 0.85, 1.0)
                } else {
                    PIP_GREEN
                };
                (f, c)
            }
            Stat::Rads => (s.rads / Survival::MAX_RADS, if rad_rate > 0.0 { WARN } else { PIP_GREEN }),
        };
        node.width = Val::Percent(frac.clamp(0.0, 1.0) * 100.0);
        // Pulse a bar that's in trouble.
        let pulse = if (fill.0 == Stat::Heat && s.body_heat < 30.0) || (fill.0 == Stat::Hp && frac < 0.25) {
            0.6 + 0.4 * (t * 6.0).sin().abs()
        } else {
            1.0
        };
        bg.0 = color.with_alpha(pulse);
    }
    if let Ok(mut node) = rad_loss.single_mut() {
        let lost = (Survival::BASE_MAX_HEALTH - s.max_health()) / Survival::BASE_MAX_HEALTH;
        node.width = Val::Percent(lost.clamp(0.0, 1.0) * 100.0);
    }

    // ---- Values ----
    for (mut text, mut color, v) in &mut texts.p0() {
        let (value, warn) = match v.0 {
            Stat::Hp => (format!("{:>3.0}/{:.0}", s.health, s.max_health()), s.health < 25.0),
            Stat::Heat => {
                let note = if s.frostbite {
                    " FROSTBITE"
                } else if sheltered {
                    " warming"
                } else if s.body_heat < 30.0 {
                    " freezing"
                } else {
                    ""
                };
                (format!("{:>3.0}%{}", s.body_heat, note), s.body_heat < 30.0)
            }
            Stat::Rads => {
                let rate = if rad_rate > 0.0 { format!(" +{rad_rate:.1}/s") } else { String::new() };
                (format!("{:>4.0}{}", s.rads, rate), rad_rate > 0.0)
            }
        };
        text.0 = value;
        color.0 = if warn { WARN } else { PIP_GREEN };
    }

    // ---- Conditions line ----
    let feels = Survival::effective_temp(&Exposure {
        air_temp_f: air_temp,
        wind_chill_f: cond.wind_chill_f,
        sheltered,
        ..Default::default()
    });
    let phase_line = match w.phase {
        Phase::Calm => format!("Calm ({:.0}s)", w.timer.max(0.0)),
        Phase::Warning => format!("SIREN - blizzard in {:.0}s", w.timer.max(0.0)),
        Phase::Blizzard => format!("RAD-BLIZZARD ({:.0}s left)", w.timer.max(0.0)),
    };
    if let Ok(mut text) = texts.p1().single_mut() {
        text.0 = format!(
            "{:.0}F  feels {:.0}F  |  {}\nDay {}  {}  {}",
            air_temp,
            feels,
            phase_line,
            clock.0.day,
            clock.0.label(),
            if clock.0.is_night() { "Night" } else { "Day" }
        );
    }

    // ---- Weapon + inventory ----
    let wpn = &game.weapon;
    let wstate = if wpn.jammed && wpn.is_reloading() {
        "UNJAMMING"
    } else if wpn.jammed {
        "JAMMED [R]"
    } else if wpn.is_reloading() {
        "RELOADING"
    } else {
        ""
    };
    if let Ok((mut text, mut color)) = texts.p3().single_mut() {
        text.0 = format!("{:>2} / {}", wpn.mag, game.inv.ammo_reserve);
        color.0 = if wpn.jammed || wpn.mag == 0 { WARN } else { PIP_GREEN };
    }
    let inv = &game.inv;
    let coat = if inv.has_frostfang_coat {
        "Frostfang coat: WORN".to_string()
    } else {
        format!("Pelts {}/{} for a coat", inv.pelts, Inventory::PELTS_FOR_COAT)
    };
    if let Ok(mut text) = texts.p2().single_mut() {
        text.0 = format!(
            "{}  {}\nStimpak x{}  RadAway x{}  Hotdish x{}\n{}  |  Kills {}",
            wpn.name.to_uppercase(),
            wstate,
            inv.stimpaks,
            inv.radaway,
            inv.hotdish,
            coat,
            game.kills,
        );
    }

    // ---- Message line ----
    if let Ok((mut text, mut color)) = texts.p4().single_mut() {
        if msgs.timer > 0.0 && game.death.is_none() {
            text.0 = msgs.text.clone();
            color.0 = WARN.with_alpha(msgs.timer.clamp(0.0, 1.0));
        } else {
            text.0.clear();
        }
    }

    // ---- Death screen ----
    if let Ok(mut text) = texts.p5().single_mut() {
        text.0 = match game.death {
            Some(cause) => format!("YOU DIED\n\n{}\n\nPress R to return to Vault 143", cause.describe()),
            None => String::new(),
        };
    }

    // ---- Compass ----
    let heading = (-yaw).to_degrees().rem_euclid(360.0);
    let mut markers = vec![(bearing(VAULT_POS.0 - pos.x, VAULT_POS.1 - pos.z), 'V')];
    if let Some(&(sx, sz)) = SHELTERS.iter().min_by(|a, b| {
        let da = (a.0 - pos.x).hypot(a.1 - pos.z);
        let db = (b.0 - pos.x).hypot(b.1 - pos.z);
        da.total_cmp(&db)
    }) {
        markers.push((bearing(sx - pos.x, sz - pos.z), 'H'));
    }
    if let Ok(mut text) = texts.p6().single_mut() {
        text.0 = compass_strip(heading, &markers);
    }
    if let Ok(mut text) = texts.p7().single_mut() {
        let deg = (heading.round() as i32).rem_euclid(360);
        text.0 = format!("{deg:03}   V vault   H shelter");
    }

    if let Ok(mut c) = help.single_mut() {
        c.0 = PIP_GREEN.with_alpha((0.6 - (t - 45.0) * 0.05).clamp(0.0, 0.6));
    }

    // ---- Overlays ----
    let hurt_alpha = if game.death.is_some() { 0.45 } else { game.hurt_flash * 0.35 };
    if let Ok(mut bg) = overlays.p0().single_mut() {
        bg.0 = Color::srgba(0.8, 0.0, 0.0, hurt_alpha);
    }
    let frost_alpha = ((50.0 - s.body_heat) / 50.0).clamp(0.0, 1.0);
    if let Ok(mut img) = overlays.p1().single_mut() {
        img.color = Color::srgba(1.0, 1.0, 1.0, frost_alpha * 0.9);
    }
    let rad_alpha = if rad_rate > 0.0 {
        (rad_rate / 15.0).clamp(0.0, 1.0) * (0.06 + 0.03 * (t * 3.0).sin())
    } else {
        0.0
    };
    if let Ok(mut bg) = overlays.p2().single_mut() {
        bg.0 = Color::srgba(0.2, 1.0, 0.2, rad_alpha);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bearings_follow_the_map() {
        assert!((bearing(0.0, -1.0) - 0.0).abs() < 1e-3, "north is -Z");
        assert!((bearing(1.0, 0.0) - 90.0).abs() < 1e-3, "east is +X");
        assert!((bearing(0.0, 1.0) - 180.0).abs() < 1e-3);
        assert!((bearing(-1.0, 0.0) - 270.0).abs() < 1e-3);
    }

    #[test]
    fn compass_centres_on_heading() {
        let strip = compass_strip(0.0, &[]);
        assert_eq!(strip.chars().count(), 61);
        assert_eq!(strip.chars().nth(30), Some('N'));
        assert_eq!(strip.chars().nth(30 + 45), None);
        let east = compass_strip(90.0, &[(100.0, 'V')]);
        assert_eq!(east.chars().nth(30), Some('E'));
        assert_eq!(east.chars().nth(35), Some('V'), "a marker 10 degrees right sits 5 chars right");
    }
}
