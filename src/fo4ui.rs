//! The on-screen furniture that tells you what you're getting: the loot list
//! that opens beside the crosshair when you look at a container (corner
//! brackets round the reticle, a title, tagged entries with the highlight on
//! one, and the buttons under it), the little feed of things you've just
//! picked up, the XP bar that slides up when you earn experience, and the big
//! LEVEL UP banner. Styled in the winter palette (see `theme.rs`).

use bevy::prelude::*;

use crate::assets::GameAssets;
use crate::interact::{Container, LootMenu};
use crate::pipboy::{total_upgrades, PipState};
use crate::saves::{Loaded, StoryFlags};
use crate::sim::lootmenu::{self, ROWS};
use crate::sim::progress::{self, Breakdown};
use crate::sim::quest as rules;
use crate::sim::synth::Sound;
use crate::state::{Game, SfxQueue};
use crate::theme::{ACCENT, ACCENT_DIM, FROST, HUD_PANEL, NAVY, SELECTED, SELECTED_FILL};

// ---------------------------------------------------------------------------
// The feed of things picked up
// ---------------------------------------------------------------------------

/// Lines like "Pipe rounds (12) added" that stack up at the left and fade.
#[derive(Resource, Default)]
pub struct Feed(pub Vec<FeedLine>);

pub struct FeedLine {
    pub text: String,
    pub age: f32,
}

impl Feed {
    pub fn push(&mut self, text: impl Into<String>) {
        self.0.push(FeedLine { text: text.into(), age: 0.0 });
        // Keep the newest few.
        let excess = self.0.len().saturating_sub(FEED_ROWS);
        self.0.drain(..excess);
    }
}

const FEED_ROWS: usize = 5;
const FEED_SECS: f32 = 4.5;

// ---------------------------------------------------------------------------
// XP
// ---------------------------------------------------------------------------

/// The XP bar and level-up banner's state.
#[derive(Resource, Default)]
struct XpPopup {
    /// Seconds left showing the bar.
    timer: f32,
    gain: u32,
    reason: &'static str,
    /// The bar's fill as drawn (it eases to the real value).
    shown: f32,
    /// What it was before the gain, for easing from.
    level: u32,
    /// Seconds left of the LEVEL UP banner.
    banner: f32,
}

const XP_SECS: f32 = 5.0;
const BANNER_SECS: f32 = 3.6;

#[derive(Component)]
struct LootRoot;
#[derive(Component)]
struct LootTitle;
#[derive(Component)]
struct LootRow(usize);
#[derive(Component)]
struct LootMore;
#[derive(Component)]
struct Bracket;
#[derive(Component)]
struct FeedRow(usize);
#[derive(Component)]
struct XpRoot;
#[derive(Component)]
struct XpGain;
#[derive(Component)]
struct XpLevelFrom;
#[derive(Component)]
struct XpLevelTo;
#[derive(Component)]
struct XpFill;
#[derive(Component)]
struct BannerRoot;
#[derive(Component)]
struct BannerLevel;

pub struct Fo4UiPlugin;

impl Plugin for Fo4UiPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Feed>()
            .init_resource::<XpPopup>()
            .add_systems(Startup, build_ui)
            .add_systems(Update, (xp_watch, render_loot, render_feed, render_xp));
    }
}

fn build_ui(mut commands: Commands, assets: Res<GameAssets>) {
    let font = |size: f32| TextFont { font: assets.font.clone(), font_size: size, ..default() };

    // ---- The loot list, to the right of the crosshair ----
    commands
        .spawn((
            Node { position_type: PositionType::Absolute, left: Val::Percent(52.5), top: Val::Percent(46.0), flex_direction: FlexDirection::Column, row_gap: Val::Px(3.0), width: Val::Px(310.0), display: Display::None, ..default() },
            GlobalZIndex(60),
            LootRoot,
        ))
        .with_children(|root| {
            root.spawn((Text::new(""), font(20.0), TextColor(FROST), LootTitle));
            for i in 0..ROWS {
                root.spawn((
                    Node { padding: UiRect::axes(Val::Px(10.0), Val::Px(5.0)), border: UiRect::all(Val::Px(1.0)), display: Display::None, ..default() },
                    BackgroundColor(HUD_PANEL),
                    BorderColor(ACCENT_DIM),
                    Text::new(""),
                    font(17.0),
                    TextColor(ACCENT),
                    LootRow(i),
                ));
            }
            root.spawn((Text::new(""), font(13.0), TextColor(ACCENT_DIM), LootMore));
            // The buttons.
            root.spawn(Node { column_gap: Val::Px(20.0), margin: UiRect::top(Val::Px(8.0)), align_items: AlignItems::Center, ..default() }).with_children(|row| {
                for (key, label) in [("E", "TAKE"), ("T", "TAKE ALL"), ("^v", "SELECT")] {
                    row.spawn(Node { column_gap: Val::Px(6.0), align_items: AlignItems::Center, ..default() }).with_children(|b| {
                        b.spawn((
                            Node { width: Val::Px(26.0), height: Val::Px(26.0), border: UiRect::all(Val::Px(2.0)), align_items: AlignItems::Center, justify_content: JustifyContent::Center, ..default() },
                            BorderColor(SELECTED),
                            BorderRadius::MAX,
                            BackgroundColor(HUD_PANEL),
                        ))
                        .with_children(|c| {
                            c.spawn((Text::new(key), font(13.0), TextColor(SELECTED)));
                        });
                        b.spawn((Text::new(label), font(15.0), TextColor(FROST)));
                    });
                }
            });
        });

    // ---- Corner brackets round the reticle ----
    commands
        .spawn(Node { position_type: PositionType::Absolute, width: Val::Percent(100.0), height: Val::Percent(100.0), align_items: AlignItems::Center, justify_content: JustifyContent::Center, ..default() })
        .with_children(|p| {
            p.spawn((Node { width: Val::Px(46.0), height: Val::Px(46.0), display: Display::None, ..default() }, Bracket)).with_children(|b| {
                for (left, top, border) in [
                    (true, true, UiRect { left: Val::Px(2.0), top: Val::Px(2.0), ..default() }),
                    (false, true, UiRect { right: Val::Px(2.0), top: Val::Px(2.0), ..default() }),
                    (true, false, UiRect { left: Val::Px(2.0), bottom: Val::Px(2.0), ..default() }),
                    (false, false, UiRect { right: Val::Px(2.0), bottom: Val::Px(2.0), ..default() }),
                ] {
                    b.spawn((
                        Node {
                            position_type: PositionType::Absolute,
                            width: Val::Px(12.0),
                            height: Val::Px(12.0),
                            left: if left { Val::Px(0.0) } else { Val::Auto },
                            right: if left { Val::Auto } else { Val::Px(0.0) },
                            top: if top { Val::Px(0.0) } else { Val::Auto },
                            bottom: if top { Val::Auto } else { Val::Px(0.0) },
                            border,
                            ..default()
                        },
                        BorderColor(SELECTED),
                    ));
                }
            });
        });

    // ---- The feed of pickups ----
    commands.spawn(Node { position_type: PositionType::Absolute, left: Val::Px(14.0), bottom: Val::Px(172.0), flex_direction: FlexDirection::ColumnReverse, row_gap: Val::Px(4.0), ..default() }).with_children(|col| {
        for i in 0..FEED_ROWS {
            col.spawn((
                Node { padding: UiRect::axes(Val::Px(10.0), Val::Px(4.0)), border: UiRect::left(Val::Px(3.0)), display: Display::None, ..default() },
                BackgroundColor(HUD_PANEL),
                BorderColor(SELECTED),
                Text::new(""),
                font(16.0),
                TextColor(FROST),
                FeedRow(i),
            ));
        }
    });

    // ---- The XP bar, bottom centre ----
    commands
        .spawn((
            Node { position_type: PositionType::Absolute, bottom: Val::Px(26.0), width: Val::Percent(100.0), flex_direction: FlexDirection::Column, align_items: AlignItems::Center, row_gap: Val::Px(5.0), display: Display::None, ..default() },
            GlobalZIndex(55),
            XpRoot,
        ))
        .with_children(|root| {
            root.spawn((Text::new(""), font(20.0), TextColor(FROST), XpGain));
            root.spawn(Node { align_items: AlignItems::Center, column_gap: Val::Px(12.0), padding: UiRect::axes(Val::Px(14.0), Val::Px(6.0)), ..default() })
                .insert(BackgroundColor(HUD_PANEL))
                .with_children(|row| {
                    row.spawn((Text::new(""), font(15.0), TextColor(ACCENT), XpLevelFrom));
                    row.spawn((Node { width: Val::Px(340.0), height: Val::Px(9.0), border: UiRect::all(Val::Px(1.0)), padding: UiRect::all(Val::Px(1.0)), ..default() }, BorderColor(ACCENT_DIM))).with_children(|bar| {
                        bar.spawn((Node { width: Val::Percent(0.0), height: Val::Percent(100.0), ..default() }, BackgroundColor(SELECTED), XpFill));
                    });
                    row.spawn((Text::new(""), font(15.0), TextColor(ACCENT), XpLevelTo));
                });
        });

    // ---- LEVEL UP ----
    commands
        .spawn((
            Node { position_type: PositionType::Absolute, top: Val::Percent(17.0), width: Val::Percent(100.0), flex_direction: FlexDirection::Column, align_items: AlignItems::Center, row_gap: Val::Px(4.0), display: Display::None, ..default() },
            GlobalZIndex(58),
            BannerRoot,
        ))
        .with_children(|root| {
            root.spawn(Node { width: Val::Px(360.0), height: Val::Px(2.0), ..default() }).insert(BackgroundColor(SELECTED));
            root.spawn((Text::new("LEVEL UP"), font(54.0), TextColor(FROST)));
            root.spawn((Text::new(""), font(24.0), TextColor(SELECTED), BannerLevel));
            root.spawn(Node { width: Val::Px(360.0), height: Val::Px(2.0), ..default() }).insert(BackgroundColor(SELECTED));
        });
}

/// What each level's XP is made of, right now.
pub(crate) fn breakdown(game: &Game, pip: &PipState, flags: &rules::Flags) -> Breakdown {
    let places = pip.fog.landmarks_found.iter().filter(|f| **f).count();
    Breakdown::new(game.kills, places, total_upgrades(game), game.inv.has_frostfang_coat, rules::quest_xp(flags))
}

/// Notice XP coming in: slide the bar up, and announce a new level.
fn xp_watch(
    time: Res<Time<Real>>,
    game: Res<Game>,
    pip: Res<PipState>,
    flags: Res<StoryFlags>,
    mut loaded: EventReader<Loaded>,
    mut popup: ResMut<XpPopup>,
    mut sfx: ResMut<SfxQueue>,
    mut perk_delay: ResMut<crate::quest::PerkDelay>,
    mut last: Local<Option<Breakdown>>,
) {
    let now = breakdown(&game, &pip, &flags.0);
    let dt = time.delta_secs();
    popup.timer = (popup.timer - dt).max(0.0);
    popup.banner = (popup.banner - dt).max(0.0);
    // A load isn't a gain: start counting again from where the save is.
    if loaded.read().count() > 0 || last.is_none() {
        *last = Some(now);
        popup.shown = progress::level(now.total()).1 as f32 / progress::level(now.total()).2.max(1) as f32;
        popup.level = progress::level(now.total()).0;
        return;
    }
    let before = last.replace(now).unwrap_or(now);
    if let Some((amount, reason)) = progress::gain(&before, &now) {
        let (old_level, new_level) = (progress::level(before.total()), progress::level(now.total()));
        // The bar starts from where it was (the bottom of the new level if one was gained).
        popup.shown = if new_level.0 > old_level.0 { 0.0 } else { old_level.1 as f32 / old_level.2.max(1) as f32 };
        // Gains that arrive while the bar is up add together.
        popup.gain = if popup.timer > 0.0 { popup.gain + amount } else { amount };
        popup.timer = XP_SECS;
        popup.reason = reason;
        popup.level = old_level.0;
        if new_level.0 > old_level.0 {
            popup.banner = BANNER_SECS;
            // The perk choice waits until the banner has had its moment.
            perk_delay.0 = BANNER_SECS + 0.8;
            sfx.play(Sound::PickupMed);
            sfx.play(Sound::PipOn);
        } else {
            sfx.play(Sound::UiTab);
        }
    }
}

fn render_xp(
    time: Res<Time<Real>>,
    game: Res<Game>,
    pip: Res<PipState>,
    flags: Res<StoryFlags>,
    mut popup: ResMut<XpPopup>,
    mut root: Query<&mut Node, (With<XpRoot>, Without<XpFill>, Without<BannerRoot>)>,
    mut gain: Query<(&mut Text, &mut TextColor), (With<XpGain>, Without<XpLevelFrom>, Without<XpLevelTo>, Without<BannerLevel>)>,
    mut from: Query<&mut Text, (With<XpLevelFrom>, Without<XpGain>, Without<XpLevelTo>, Without<BannerLevel>)>,
    mut to: Query<&mut Text, (With<XpLevelTo>, Without<XpGain>, Without<XpLevelFrom>, Without<BannerLevel>)>,
    mut fill: Query<&mut Node, (With<XpFill>, Without<XpRoot>, Without<BannerRoot>)>,
    mut banner: Query<&mut Node, (With<BannerRoot>, Without<XpRoot>, Without<XpFill>)>,
    mut banner_text: Query<&mut Text, (With<BannerLevel>, Without<XpGain>, Without<XpLevelFrom>, Without<XpLevelTo>)>,
) {
    let (Ok(mut root), Ok(mut banner)) = (root.single_mut(), banner.single_mut()) else { return };
    root.display = if popup.timer > 0.0 { Display::Flex } else { Display::None };
    banner.display = if popup.banner > 0.0 { Display::Flex } else { Display::None };
    if popup.timer <= 0.0 && popup.banner <= 0.0 {
        return;
    }
    let xp = breakdown(&game, &pip, &flags.0).total();
    let (level, into, span) = progress::level(xp);
    let target = into as f32 / span.max(1) as f32;
    // Ease the bar up to the new value.
    let k = (time.delta_secs() * 2.2).min(1.0);
    popup.shown += (target - popup.shown) * k;
    if let Ok(mut f) = fill.single_mut() {
        f.width = Val::Percent((popup.shown * 100.0).clamp(0.0, 100.0));
    }
    if let Ok((mut t, mut c)) = gain.single_mut() {
        t.set_if_neq(Text::new(format!("+{} XP   {}", popup.gain, popup.reason.to_uppercase())));
        c.set_if_neq(TextColor(FROST.with_alpha((popup.timer / 0.8).clamp(0.0, 1.0))));
    }
    if let Ok(mut t) = from.single_mut() {
        t.set_if_neq(Text::new(format!("LEVEL {level}")));
    }
    if let Ok(mut t) = to.single_mut() {
        t.set_if_neq(Text::new(format!("{}", level + 1)));
    }
    if let Ok(mut t) = banner_text.single_mut() {
        t.set_if_neq(Text::new(format!("YOU ARE NOW LEVEL {level}")));
    }
}

// ---------------------------------------------------------------------------
// Drawing the loot list and the feed
// ---------------------------------------------------------------------------

#[allow(clippy::too_many_arguments)]
fn render_loot(
    menu: Res<LootMenu>,
    containers: Query<&Container>,
    mut root: Query<&mut Node, (With<LootRoot>, Without<LootRow>, Without<Bracket>)>,
    mut title: Query<&mut Text, (With<LootTitle>, Without<LootRow>, Without<LootMore>)>,
    mut more: Query<&mut Text, (With<LootMore>, Without<LootRow>, Without<LootTitle>)>,
    mut rows: Query<(&LootRow, &mut Node, &mut Text, &mut TextColor, &mut BackgroundColor, &mut BorderColor), (Without<LootRoot>, Without<LootTitle>, Without<LootMore>, Without<Bracket>)>,
    mut bracket: Query<&mut Node, (With<Bracket>, Without<LootRoot>, Without<LootRow>)>,
) {
    let (Ok(mut root), Ok(mut bracket)) = (root.single_mut(), bracket.single_mut()) else { return };
    let container = menu.target.and_then(|e| containers.get(e).ok());
    let Some(c) = container else {
        root.display = Display::None;
        bracket.display = Display::None;
        return;
    };
    root.display = Display::Flex;
    bracket.display = Display::Flex;
    let left = c.remaining();
    let (from, to) = lootmenu::window(menu.selected, left.len());
    if let Ok(mut t) = title.single_mut() {
        t.set_if_neq(Text::new(c.name.to_uppercase()));
    }
    if let Ok(mut t) = more.single_mut() {
        let hidden = left.len().saturating_sub(to - from);
        t.set_if_neq(Text::new(if hidden > 0 { format!("+{hidden} more") } else { String::new() }));
    }
    for (row, mut node, mut text, mut color, mut bg, mut border) in &mut rows {
        let slot = from + row.0;
        match left.get(slot).filter(|_| slot < to) {
            Some(&i) => {
                node.display = Display::Flex;
                let entry = &c.loot[i];
                text.set_if_neq(Text::new(format!("[{}] {}", lootmenu::category(entry), lootmenu::label(entry))));
                let selected = slot == menu.selected;
                // The highlighted entry is lit solid; the rest are dark plates.
                color.set_if_neq(TextColor(if selected { NAVY } else { ACCENT }));
                bg.set_if_neq(BackgroundColor(if selected { SELECTED } else { HUD_PANEL }));
                border.set_if_neq(BorderColor(if selected { SELECTED } else { ACCENT_DIM }));
            }
            None => node.display = Display::None,
        }
    }
}

fn render_feed(time: Res<Time<Real>>, mut feed: ResMut<Feed>, mut rows: Query<(&FeedRow, &mut Node, &mut Text, &mut TextColor, &mut BackgroundColor, &mut BorderColor)>) {
    let dt = time.delta_secs();
    for line in &mut feed.0 {
        line.age += dt;
    }
    feed.0.retain(|l| l.age < FEED_SECS);
    // Newest at the bottom: row 0 is the newest.
    let n = feed.0.len();
    for (row, mut node, mut text, mut color, mut bg, mut border) in &mut rows {
        match n.checked_sub(1 + row.0).and_then(|i| feed.0.get(i)) {
            Some(line) => {
                node.display = Display::Flex;
                text.set_if_neq(Text::new(line.text.clone()));
                let fade = ((FEED_SECS - line.age) / 0.8).clamp(0.0, 1.0);
                // A new line flashes bright, then settles.
                let flash = (1.0 - line.age / 0.35).clamp(0.0, 1.0);
                color.set_if_neq(TextColor(FROST.with_alpha(fade)));
                bg.set_if_neq(BackgroundColor(if flash > 0.0 { SELECTED_FILL.with_alpha(0.55 * fade) } else { HUD_PANEL.with_alpha(HUD_PANEL.alpha() * fade) }));
                border.set_if_neq(BorderColor(SELECTED.with_alpha(fade)));
            }
            None => node.display = Display::None,
        }
    }
}
