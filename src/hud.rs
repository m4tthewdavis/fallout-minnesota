//! Pip-Boy green HUD: health / Body Heat / rads bars, weather, weapon,
//! inventory, messages, crosshair, and damage / frost / death overlays.

use bevy::prelude::*;

use crate::player::Player;
use crate::sim::survival::{Inventory, Survival};
use crate::sim::terrain;
use crate::sim::weather::Phase;
use crate::state::{Game, Messages, WeatherRes};

const PIP_GREEN: Color = Color::srgb(0.45, 1.0, 0.45);
const WARN: Color = Color::srgb(1.0, 0.75, 0.3);

#[derive(Component)]
struct StatusText;
#[derive(Component)]
struct WeaponText;
#[derive(Component)]
struct MessageText;
#[derive(Component)]
struct DeathText;
#[derive(Component)]
struct HurtOverlay;
#[derive(Component)]
struct FrostOverlay;

pub struct HudPlugin;

impl Plugin for HudPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_hud).add_systems(Update, update_hud);
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

fn spawn_hud(mut commands: Commands) {
    let font = |size: f32| TextFont {
        font_size: size,
        ..default()
    };

    // Overlays first so text draws on top of them.
    commands.spawn((
        full_screen(),
        BackgroundColor(Color::srgba(0.7, 0.85, 1.0, 0.0)),
        FrostOverlay,
    ));
    commands.spawn((
        full_screen(),
        BackgroundColor(Color::srgba(0.8, 0.0, 0.0, 0.0)),
        HurtOverlay,
    ));

    // Crosshair.
    commands.spawn(full_screen()).with_children(|p| {
        p.spawn((Text::new("+"), font(26.0), TextColor(PIP_GREEN)));
    });

    // Controls reminder.
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
    ));

    commands.spawn((
        Text::new(""),
        font(17.0),
        TextColor(PIP_GREEN),
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(14.0),
            bottom: Val::Px(12.0),
            ..default()
        },
        StatusText,
    ));

    commands.spawn((
        Text::new(""),
        font(17.0),
        TextColor(PIP_GREEN),
        TextLayout::new_with_justify(JustifyText::Right),
        Node {
            position_type: PositionType::Absolute,
            right: Val::Px(14.0),
            bottom: Val::Px(12.0),
            ..default()
        },
        WeaponText,
    ));

    // Top-centre message line.
    commands
        .spawn(Node {
            position_type: PositionType::Absolute,
            width: Val::Percent(100.0),
            top: Val::Px(60.0),
            justify_content: JustifyContent::Center,
            ..default()
        })
        .with_children(|p| {
            p.spawn((
                Text::new(""),
                font(20.0),
                TextColor(WARN),
                TextLayout::new_with_justify(JustifyText::Center),
                MessageText,
            ));
        });

    // Death screen text.
    commands.spawn(full_screen()).with_children(|p| {
        p.spawn((
            Text::new(""),
            font(30.0),
            TextColor(Color::srgb(1.0, 0.35, 0.3)),
            TextLayout::new_with_justify(JustifyText::Center),
            DeathText,
        ));
    });
}

fn bar(value: f32, max: f32, width: usize) -> String {
    let frac = if max > 0.0 { (value / max).clamp(0.0, 1.0) } else { 0.0 };
    let filled = (frac * width as f32).round() as usize;
    format!("[{}{}]", "|".repeat(filled), ".".repeat(width - filled))
}

#[allow(clippy::type_complexity)]
fn update_hud(
    game: Res<Game>,
    weather: Res<WeatherRes>,
    msgs: Res<Messages>,
    player: Query<&Transform, With<Player>>,
    mut texts: ParamSet<(
        Query<&mut Text, With<StatusText>>,
        Query<&mut Text, With<WeaponText>>,
        Query<(&mut Text, &mut TextColor), With<MessageText>>,
        Query<&mut Text, With<DeathText>>,
    )>,
    mut overlays: ParamSet<(
        Query<&mut BackgroundColor, With<HurtOverlay>>,
        Query<&mut BackgroundColor, With<FrostOverlay>>,
    )>,
) {
    let s = &game.survival;
    let w = &weather.weather;
    let cond = w.conditions();

    let (sheltered, ambient_rads) = player
        .single()
        .map(|t| {
            let (x, z) = (t.translation.x, t.translation.z);
            (terrain::shelter_at(x, z).is_some(), terrain::ambient_rads(x, z))
        })
        .unwrap_or((false, 0.0));

    // ---- Status (bottom-left) ----
    let heat_note = if s.frostbite {
        "  FROSTBITE!"
    } else if sheltered {
        "  warming by the fire"
    } else if s.body_heat < 30.0 {
        "  freezing"
    } else {
        ""
    };
    let rad_rate = ambient_rads + if sheltered { 0.0 } else { cond.rads_per_sec };
    let rad_note = if rad_rate > 0.0 {
        format!("  +{rad_rate:.1}/s")
    } else {
        String::new()
    };
    let feels = Survival::effective_temp(&crate::sim::survival::Exposure {
        air_temp_f: cond.air_temp_f,
        wind_chill_f: cond.wind_chill_f,
        sheltered,
        ..Default::default()
    });
    let phase_line = match w.phase {
        Phase::Calm => format!("Calm  ({:.0}s)", w.timer.max(0.0)),
        Phase::Warning => format!("SIREN - blizzard in {:.0}s", w.timer.max(0.0)),
        Phase::Blizzard => format!("RAD-BLIZZARD  ({:.0}s left)", w.timer.max(0.0)),
    };
    let status = format!(
        "HP    {} {:>3.0}/{:.0}\nHEAT  {} {:>3.0}%{}\nRADS  {} {:>4.0}{}\n{:.0}F  feels like {:.0}F  |  {}",
        bar(s.health, Survival::BASE_MAX_HEALTH, 20),
        s.health,
        s.max_health(),
        bar(s.body_heat, 100.0, 20),
        s.body_heat,
        heat_note,
        bar(s.rads, Survival::MAX_RADS, 20),
        s.rads,
        rad_note,
        cond.air_temp_f,
        feels,
        phase_line,
    );
    if let Ok(mut t) = texts.p0().single_mut() {
        t.0 = status;
    }

    // ---- Weapon + inventory (bottom-right) ----
    let wpn = &game.weapon;
    let wstate = if wpn.jammed && wpn.is_reloading() {
        "  UNJAMMING"
    } else if wpn.jammed {
        "  JAMMED [R]"
    } else if wpn.is_reloading() {
        "  RELOADING"
    } else {
        ""
    };
    let inv = &game.inv;
    let coat = if inv.has_frostfang_coat {
        "Frostfang coat: WORN".to_string()
    } else {
        format!("Pelts {}/{} for a coat", inv.pelts, Inventory::PELTS_FOR_COAT)
    };
    let weapon_text = format!(
        "{}  {}/{}  |  {}{}\nStimpak x{}  RadAway x{}  Hotdish x{}\n{}  |  Kills {}",
        wpn.name.to_uppercase(),
        wpn.mag,
        wpn.mag_size,
        inv.ammo_reserve,
        wstate,
        inv.stimpaks,
        inv.radaway,
        inv.hotdish,
        coat,
        game.kills,
    );
    if let Ok(mut t) = texts.p1().single_mut() {
        t.0 = weapon_text;
    }

    // ---- Message line ----
    if let Ok((mut t, mut color)) = texts.p2().single_mut() {
        if msgs.timer > 0.0 && game.death.is_none() {
            t.0 = msgs.text.clone();
            color.0 = WARN.with_alpha(msgs.timer.clamp(0.0, 1.0));
        } else {
            t.0.clear();
        }
    }

    // ---- Death screen ----
    if let Ok(mut t) = texts.p3().single_mut() {
        t.0 = match game.death {
            Some(cause) => format!("YOU DIED\n\n{}\n\nPress R to return to Vault 143", cause.describe()),
            None => String::new(),
        };
    }

    // ---- Overlays ----
    let hurt_alpha = if game.death.is_some() {
        0.45
    } else {
        game.hurt_flash * 0.35
    };
    if let Ok(mut bg) = overlays.p0().single_mut() {
        bg.0 = Color::srgba(0.8, 0.0, 0.0, hurt_alpha);
    }
    let frost_alpha = ((40.0 - s.body_heat) / 40.0).clamp(0.0, 1.0) * 0.35;
    if let Ok(mut bg) = overlays.p1().single_mut() {
        bg.0 = Color::srgba(0.7, 0.85, 1.0, frost_alpha);
    }
}
