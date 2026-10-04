//! The pause menu as pure rules: which screen you're on, what rows it shows,
//! how the selection moves, and what choosing a row does. `menu.rs` draws it
//! and carries out the [`Action`]s it returns.

use super::settings::{Settings, ShadowQuality, ViewDistance, UI_SCALE_MAX, UI_SCALE_MIN};

/// Autosave plus three manual slots.
pub const SLOTS: usize = 4;
/// Slot 0 is the autosave; the manual slots are 1..=3.
pub const AUTOSAVE: usize = 0;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Screen {
    Main,
    Settings,
    Save,
    Load,
    ConfirmQuit,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RowKind {
    Button,
    /// Left/right (or Enter) picks another value.
    Choice,
    /// Left/right moves a bar.
    Slider,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Row {
    pub label: String,
    /// What it's set to (empty for plain buttons).
    pub value: String,
    pub kind: RowKind,
    pub enabled: bool,
}

/// What the game must do after a menu input.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Action {
    None,
    Resume,
    Save(usize),
    Load(usize),
    Quit,
}

/// What the menu needs to know about the game to enable its rows.
#[derive(Clone, Debug)]
pub struct Context {
    /// The player is alive (you can't save a dead character).
    pub alive: bool,
    /// One line describing each save slot, or `None` if it's empty.
    pub slots: [Option<String>; SLOTS],
}

impl Context {
    pub fn any_save(&self) -> bool {
        self.slots.iter().any(|s| s.is_some())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Menu {
    pub screen: Screen,
    pub selected: usize,
}

impl Default for Menu {
    fn default() -> Self {
        Menu { screen: Screen::Main, selected: 0 }
    }
}

fn button(label: &str, enabled: bool) -> Row {
    Row { label: label.to_string(), value: String::new(), kind: RowKind::Button, enabled }
}

fn choice(label: &str, value: &str) -> Row {
    Row { label: label.to_string(), value: format!("< {value} >"), kind: RowKind::Choice, enabled: true }
}

fn slider(label: &str, fraction: f32, text: String) -> Row {
    let n = (fraction.clamp(0.0, 1.0) * 10.0).round() as usize;
    Row { label: label.to_string(), value: format!("< {}{} {text} >", "#".repeat(n), "-".repeat(10 - n)), kind: RowKind::Slider, enabled: true }
}

fn volume(label: &str, v: f32) -> Row {
    slider(label, v, format!("{:>3.0}%", v * 100.0))
}

fn slot_name(slot: usize) -> String {
    if slot == AUTOSAVE {
        "Autosave".to_string()
    } else {
        format!("Slot {slot}")
    }
}

impl Menu {
    pub fn title(&self) -> &'static str {
        match self.screen {
            Screen::Main => "PAUSED",
            Screen::Settings => "SETTINGS",
            Screen::Save => "SAVE GAME",
            Screen::Load => "LOAD GAME",
            Screen::ConfirmQuit => "QUIT TO DESKTOP?",
        }
    }

    /// A line of explanation under the title, if the screen has one.
    pub fn note(&self) -> &'static str {
        match self.screen {
            Screen::ConfirmQuit => "Anything since your last save will be lost.",
            _ => "",
        }
    }

    pub fn rows(&self, s: &Settings, ctx: &Context) -> Vec<Row> {
        match self.screen {
            Screen::Main => vec![
                button("Resume Game", true),
                button("Save Game", ctx.alive),
                button("Load Game", ctx.any_save()),
                button("Settings", true),
                button("Quit to Desktop", true),
            ],
            Screen::Settings => vec![
                choice("Shadow Quality", s.shadows.label()),
                choice("View Distance", s.view.label()),
                slider("UI Size", (s.ui_scale - UI_SCALE_MIN) / (UI_SCALE_MAX - UI_SCALE_MIN), format!("{:>3.0}%", s.ui_scale * 100.0)),
                volume("Master Volume", s.master),
                volume("Effects Volume", s.sfx),
                volume("Music Volume", s.music),
                volume("Ambience Volume", s.ambience),
                button("Back", true),
            ],
            Screen::Save => {
                let mut rows: Vec<Row> = (1..SLOTS)
                    .map(|i| Row {
                        label: slot_name(i),
                        value: ctx.slots[i].clone().unwrap_or_else(|| "- empty -".to_string()),
                        kind: RowKind::Button,
                        enabled: ctx.alive,
                    })
                    .collect();
                rows.push(button("Back", true));
                rows
            }
            Screen::Load => {
                let mut rows: Vec<Row> = (0..SLOTS)
                    .map(|i| Row {
                        label: slot_name(i),
                        value: ctx.slots[i].clone().unwrap_or_else(|| "- empty -".to_string()),
                        kind: RowKind::Button,
                        enabled: ctx.slots[i].is_some(),
                    })
                    .collect();
                rows.push(button("Back", true));
                rows
            }
            Screen::ConfirmQuit => vec![button("No, keep playing", true), button("Yes, quit", true)],
        }
    }

    fn go(&mut self, screen: Screen, selected: usize) {
        self.screen = screen;
        self.selected = selected;
    }

    /// Put the selection on the first row you can choose.
    fn select_first_enabled(&mut self, rows: &[Row]) {
        self.selected = rows.iter().position(|r| r.enabled).unwrap_or(0);
    }

    /// Move the selection `dir` (+1 down, -1 up) to the next enabled row,
    /// wrapping round the ends. Returns true if it moved.
    pub fn move_selection(&mut self, dir: i32, rows: &[Row]) -> bool {
        let n = rows.len() as i32;
        if n == 0 || !rows.iter().any(|r| r.enabled) {
            return false;
        }
        let mut i = self.selected as i32;
        for _ in 0..n {
            i = (i + dir).rem_euclid(n);
            if rows[i as usize].enabled {
                let moved = i as usize != self.selected;
                self.selected = i as usize;
                return moved;
            }
        }
        false
    }

    /// Select a row directly (the mouse hovering over it). Disabled rows
    /// can't be selected. Returns true if the selection changed.
    pub fn select_row(&mut self, row: usize, rows: &[Row]) -> bool {
        if rows.get(row).is_some_and(|r| r.enabled) && self.selected != row {
            self.selected = row;
            true
        } else {
            false
        }
    }

    /// Esc: back out of a sub-screen, or close the menu from the main one.
    pub fn back(&mut self) -> Action {
        match self.screen {
            Screen::Main => Action::Resume,
            Screen::Settings => {
                self.go(Screen::Main, 3);
                Action::None
            }
            Screen::Save => {
                self.go(Screen::Main, 1);
                Action::None
            }
            Screen::Load => {
                self.go(Screen::Main, 2);
                Action::None
            }
            Screen::ConfirmQuit => {
                self.go(Screen::Main, 4);
                Action::None
            }
        }
    }

    /// Enter on the selected row. May change `settings` (a Choice or Slider
    /// row steps on, wrapping Choices round to the first value).
    pub fn activate(&mut self, settings: &mut Settings, ctx: &Context) -> (Action, bool) {
        let rows = self.rows(settings, ctx);
        let Some(row) = rows.get(self.selected) else { return (Action::None, false) };
        if !row.enabled {
            return (Action::None, false);
        }
        let last = rows.len() - 1;
        match self.screen {
            Screen::Main => match self.selected {
                0 => return (Action::Resume, false),
                1 => {
                    self.go(Screen::Save, 0);
                    let rows = self.rows(settings, ctx);
                    self.select_first_enabled(&rows);
                }
                2 => {
                    self.go(Screen::Load, 0);
                    let rows = self.rows(settings, ctx);
                    self.select_first_enabled(&rows);
                }
                3 => self.go(Screen::Settings, 0),
                _ => self.go(Screen::ConfirmQuit, 0),
            },
            Screen::Settings => {
                if self.selected == last {
                    self.back();
                } else {
                    let changed = self.change(1, true, settings);
                    return (Action::None, changed);
                }
            }
            Screen::Save => {
                if self.selected == last {
                    self.back();
                } else {
                    return (Action::Save(self.selected + 1), false);
                }
            }
            Screen::Load => {
                if self.selected == last {
                    self.back();
                } else {
                    return (Action::Load(self.selected), false);
                }
            }
            Screen::ConfirmQuit => {
                if self.selected == 1 {
                    return (Action::Quit, false);
                }
                self.back();
            }
        }
        (Action::None, false)
    }

    /// Left (`-1`) or right (`+1`) on a Choice or Slider row of the settings
    /// screen. Returns true if a setting changed.
    pub fn adjust(&mut self, dir: i32, settings: &mut Settings) -> bool {
        self.change(dir, false, settings)
    }

    /// `dir` is the step; with `wrap`, Enter cycling past the last choice
    /// returns to the first (and a slider at its top goes back to the bottom).
    fn change(&mut self, dir: i32, wrap: bool, s: &mut Settings) -> bool {
        if self.screen != Screen::Settings {
            return false;
        }
        let before = *s;
        match self.selected {
            0 => {
                s.shadows = if wrap && s.shadows == ShadowQuality::High { ShadowQuality::Off } else { s.shadows.step(dir) };
            }
            1 => {
                s.view = if wrap && s.view == ViewDistance::Far { ViewDistance::Near } else { s.view.step(dir) };
            }
            2 => {
                if wrap && s.ui_scale >= UI_SCALE_MAX {
                    s.ui_scale = UI_SCALE_MIN;
                } else {
                    s.nudge_ui_scale(dir);
                }
            }
            i @ 3..=6 => {
                let slot = match i {
                    3 => &mut s.master,
                    4 => &mut s.sfx,
                    5 => &mut s.music,
                    _ => &mut s.ambience,
                };
                *slot = if wrap && *slot >= 1.0 { 0.0 } else { (((*slot + dir as f32 * 0.1) * 10.0).round() / 10.0).clamp(0.0, 1.0) };
            }
            _ => {}
        }
        *s != before
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx() -> Context {
        Context { alive: true, slots: [None, None, None, None] }
    }

    fn with_saves() -> Context {
        Context { alive: true, slots: [Some("Autosave, day 2".into()), None, Some("Slot two".into()), None] }
    }

    fn rows(m: &Menu, c: &Context) -> Vec<Row> {
        m.rows(&Settings::default(), c)
    }

    #[test]
    fn the_main_menu_lists_the_five_options_in_order() {
        let r = rows(&Menu::default(), &ctx());
        let labels: Vec<_> = r.iter().map(|r| r.label.as_str()).collect();
        assert_eq!(labels, ["Resume Game", "Save Game", "Load Game", "Settings", "Quit to Desktop"]);
        assert_eq!(Menu::default().title(), "PAUSED");
    }

    #[test]
    fn saving_needs_a_live_player_and_loading_needs_a_save() {
        let r = rows(&Menu::default(), &ctx());
        assert!(r[1].enabled && !r[2].enabled, "alive, but nothing saved yet");
        let dead = Context { alive: false, ..with_saves() };
        let r = rows(&Menu::default(), &dead);
        assert!(!r[1].enabled && r[2].enabled, "can load but not save when dead");
        assert!(r[0].enabled && r[3].enabled && r[4].enabled);
    }

    #[test]
    fn selection_skips_disabled_rows_and_wraps() {
        let c = ctx(); // Load Game disabled
        let r = rows(&Menu::default(), &c);
        let mut m = Menu::default();
        assert!(m.move_selection(1, &r));
        assert_eq!(m.selected, 1);
        assert!(m.move_selection(1, &r));
        assert_eq!(m.selected, 3, "skipped the disabled Load Game");
        m.move_selection(1, &r);
        assert_eq!(m.selected, 4);
        m.move_selection(1, &r);
        assert_eq!(m.selected, 0, "wraps to the top");
        m.move_selection(-1, &r);
        assert_eq!(m.selected, 4, "and to the bottom");
    }

    #[test]
    fn a_menu_with_nothing_enabled_does_not_loop_forever() {
        let rows = vec![Row { label: "x".into(), value: String::new(), kind: RowKind::Button, enabled: false }];
        let mut m = Menu::default();
        assert!(!m.move_selection(1, &rows));
        assert!(!m.move_selection(1, &[]));
    }

    #[test]
    fn hovering_selects_only_enabled_rows() {
        let c = ctx();
        let r = rows(&Menu::default(), &c);
        let mut m = Menu::default();
        assert!(!m.select_row(2, &r), "Load Game is disabled");
        assert!(m.select_row(3, &r));
        assert!(!m.select_row(3, &r), "no change when already selected");
        assert!(!m.select_row(99, &r), "off the end is ignored");
        assert_eq!(m.selected, 3);
    }

    #[test]
    fn escape_closes_the_main_menu_and_backs_out_of_the_others() {
        let mut s = Settings::default();
        let c = with_saves();
        let mut m = Menu::default();
        assert_eq!(m.back(), Action::Resume);
        for (row, screen) in [(1, Screen::Save), (2, Screen::Load), (3, Screen::Settings), (4, Screen::ConfirmQuit)] {
            let mut m = Menu { screen: Screen::Main, selected: row };
            assert_eq!(m.activate(&mut s, &c), (Action::None, false));
            assert_eq!(m.screen, screen);
            assert_eq!(m.back(), Action::None);
            assert_eq!((m.screen, m.selected), (Screen::Main, row), "returns to the row you came from");
        }
        m.selected = 0;
        assert_eq!(m.activate(&mut s, &c).0, Action::Resume);
    }

    #[test]
    fn the_save_screen_offers_three_manual_slots() {
        let mut s = Settings::default();
        let c = with_saves();
        let mut m = Menu { screen: Screen::Save, selected: 0 };
        let r = m.rows(&s, &c);
        assert_eq!(r.len(), 4, "three slots and Back");
        assert_eq!(r[0].label, "Slot 1");
        assert_eq!(r[0].value, "- empty -");
        assert_eq!(r[1].value, "Slot two");
        assert!(r.iter().all(|r| r.enabled), "empty slots can be saved into");
        assert_eq!(m.activate(&mut s, &c).0, Action::Save(1));
        m.selected = 2;
        assert_eq!(m.activate(&mut s, &c).0, Action::Save(3));
        m.selected = 3;
        m.activate(&mut s, &c);
        assert_eq!(m.screen, Screen::Main, "Back");
        let dead = Context { alive: false, ..c };
        let m = Menu { screen: Screen::Save, selected: 0 };
        assert!(!m.rows(&s, &dead)[0].enabled, "no saving when dead");
    }

    #[test]
    fn the_load_screen_includes_the_autosave_and_only_filled_slots_load() {
        let mut s = Settings::default();
        let c = with_saves();
        let m = Menu { screen: Screen::Load, selected: 0 };
        let r = m.rows(&s, &c);
        assert_eq!(r.len(), 5);
        assert_eq!(r[0].label, "Autosave");
        assert_eq!(r.iter().map(|r| r.enabled).collect::<Vec<_>>(), [true, false, true, false, true]);
        let mut m = Menu { screen: Screen::Load, selected: 2 };
        assert_eq!(m.activate(&mut s, &c).0, Action::Load(2));
        let mut m = Menu { screen: Screen::Load, selected: 1 };
        assert_eq!(m.activate(&mut s, &c), (Action::None, false), "an empty slot does nothing");
        assert_eq!(m.screen, Screen::Load);
    }

    #[test]
    fn entering_load_selects_the_first_usable_slot() {
        let mut s = Settings::default();
        let c = Context { alive: true, slots: [None, None, Some("x".into()), None] };
        let mut m = Menu { screen: Screen::Main, selected: 2 };
        m.activate(&mut s, &c);
        assert_eq!((m.screen, m.selected), (Screen::Load, 2));
    }

    #[test]
    fn quitting_asks_first_and_defaults_to_no() {
        let mut s = Settings::default();
        let c = ctx();
        let mut m = Menu { screen: Screen::Main, selected: 4 };
        assert_eq!(m.activate(&mut s, &c).0, Action::None);
        assert_eq!((m.screen, m.selected), (Screen::ConfirmQuit, 0), "No is highlighted");
        assert_eq!(m.title(), "QUIT TO DESKTOP?");
        assert!(!m.note().is_empty());
        assert_eq!(m.activate(&mut s, &c).0, Action::None, "Enter on No goes back");
        assert_eq!(m.screen, Screen::Main);
        m.selected = 4;
        m.activate(&mut s, &c);
        m.selected = 1;
        assert_eq!(m.activate(&mut s, &c).0, Action::Quit);
    }

    #[test]
    fn left_and_right_change_the_matching_setting_and_nothing_else() {
        let c = ctx();
        let base = Settings::default();
        let mut m = Menu { screen: Screen::Settings, selected: 0 };
        let mut s = base;
        assert!(m.adjust(-1, &mut s));
        assert_eq!(s, Settings { shadows: ShadowQuality::Medium, ..base });
        m.selected = 1;
        let mut s = base;
        assert!(m.adjust(-1, &mut s));
        assert_eq!(s, Settings { view: ViewDistance::Medium, ..base });
        for (row, check) in [
            (2usize, (|s: &Settings| s.ui_scale) as fn(&Settings) -> f32),
            (3, |s| s.master),
            (4, |s| s.sfx),
            (5, |s| s.music),
            (6, |s| s.ambience),
        ] {
            m.selected = row;
            let mut s = base;
            assert!(m.adjust(-1, &mut s), "row {row}");
            assert!(check(&s) < check(&base), "row {row} went down");
            let mut others = s;
            others.master = base.master;
            others.sfx = base.sfx;
            others.music = base.music;
            others.ambience = base.ambience;
            others.ui_scale = base.ui_scale;
            assert_eq!((others.shadows, others.view), (base.shadows, base.view));
        }
    }

    #[test]
    fn arrows_stop_at_the_ends_but_enter_cycles_choices_round() {
        let c = ctx();
        let mut m = Menu { screen: Screen::Settings, selected: 0 };
        let mut s = Settings::default();
        assert!(!m.adjust(1, &mut s), "High is already the top");
        assert_eq!(s.shadows, ShadowQuality::High);
        assert_eq!(m.activate(&mut s, &c), (Action::None, true));
        assert_eq!(s.shadows, ShadowQuality::Off, "Enter wraps High to Off");
        m.activate(&mut s, &c);
        assert_eq!(s.shadows, ShadowQuality::Low);
        m.selected = 3;
        s.master = 1.0;
        assert!(!m.adjust(1, &mut s), "slider stops at 100%");
        m.activate(&mut s, &c);
        assert_eq!(s.master, 0.0, "Enter wraps a full slider back to zero");
        m.adjust(1, &mut s);
        assert_eq!(s.master, 0.1);
    }

    #[test]
    fn volume_steps_are_clean_tenths() {
        let c = ctx();
        let mut m = Menu { screen: Screen::Settings, selected: 3 };
        let mut s = Settings { master: 0.0, ..Settings::default() };
        for _ in 0..3 {
            m.adjust(1, &mut s);
        }
        assert_eq!(s.master, 0.3, "no 0.30000001");
        let r = m.rows(&s, &c);
        assert!(r[3].value.contains("###-------") && r[3].value.contains("30%"), "{}", r[3].value);
    }

    #[test]
    fn settings_rows_show_their_current_values() {
        let s = Settings { shadows: ShadowQuality::Low, view: ViewDistance::Near, ui_scale: 1.2, ..Settings::default() };
        let r = Menu { screen: Screen::Settings, selected: 0 }.rows(&s, &ctx());
        assert_eq!(r[0].value, "< Low >");
        assert_eq!(r[1].value, "< Near >");
        assert!(r[2].value.contains("120%"));
        assert_eq!(r[0].kind, RowKind::Choice);
        assert_eq!(r[3].kind, RowKind::Slider);
        assert_eq!(r.last().unwrap().label, "Back");
        assert_eq!(r.len(), 8);
    }

    #[test]
    fn adjusting_outside_the_settings_screen_does_nothing() {
        let mut s = Settings::default();
        let mut m = Menu::default();
        assert!(!m.adjust(1, &mut s));
        assert_eq!(s, Settings::default());
    }

    #[test]
    fn activating_a_disabled_row_is_ignored() {
        let mut s = Settings::default();
        let mut m = Menu { screen: Screen::Main, selected: 2 };
        assert_eq!(m.activate(&mut s, &ctx()), (Action::None, false));
        assert_eq!(m.screen, Screen::Main);
    }
}
