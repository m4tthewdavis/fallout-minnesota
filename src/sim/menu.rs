//! The pause menu as pure rules: which screen you're on, what rows it shows,
//! how the selection moves, and what choosing a row does. `menu.rs` draws it
//! and carries out the [`Action`]s it returns.

use super::keys::{self, Bind, Bindings};
use super::settings::{Settings, ShadowQuality, ViewDistance, FOV_MAX, FOV_MIN, SENSITIVITY_MAX, SENSITIVITY_MIN, UI_SCALE_MAX, UI_SCALE_MIN};

/// Autosave, quicksave and three manual slots.
pub const SLOTS: usize = 5;
/// Slot 0 is the autosave (when you sleep or take shelter), slot 1 the
/// quicksave (F4); the manual slots are 2..=4.
pub const AUTOSAVE: usize = 0;
pub const QUICKSAVE: usize = 1;
pub const FIRST_MANUAL: usize = 2;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Screen {
    /// The title screen, before the game starts.
    Title,
    /// The pause menu.
    Main,
    /// The settings hub: graphics, sound, controls.
    Settings,
    Graphics,
    Sound,
    Controls,
    Keys,
    Save,
    Load,
    ConfirmQuit,
}

/// What a row is, so choosing it does the right thing wherever it sits.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RowId {
    Resume,
    Continue,
    NewGame,
    SaveGame,
    LoadGame,
    Settings,
    Quit,
    Back,
    Graphics,
    Sound,
    Controls,
    Keys,
    Shadows,
    View,
    Volumetrics,
    AmbientOcclusion,
    ShowFps,
    UiScale,
    Master,
    Sfx,
    Music,
    Ambience,
    Sensitivity,
    InvertY,
    Fov,
    Bind(Bind),
    ResetKeys,
    Slot(usize),
    No,
    Yes,
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
    pub id: RowId,
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
    /// Leave the title screen and start playing.
    NewGame,
    Save(usize),
    Load(usize),
    Quit,
    /// Wait for the next key press and put this action on it.
    Rebind(Bind),
}

/// What the menu needs to know about the game to enable its rows.
#[derive(Clone, Debug)]
pub struct Context {
    /// The player is alive (you can't save a dead character).
    pub alive: bool,
    /// One line describing each save slot, or `None` if it's empty.
    pub slots: [Option<String>; SLOTS],
    /// The most recently written slot (for Continue).
    pub latest: Option<usize>,
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
    /// Opened from the title screen (so Back returns there, not to the pause menu).
    pub from_title: bool,
}

impl Default for Menu {
    fn default() -> Self {
        Menu { screen: Screen::Main, selected: 0, from_title: false }
    }
}

/// Rows a screen shows at once; longer lists scroll.
pub const VISIBLE_ROWS: usize = 10;

fn on_off(v: bool) -> &'static str {
    if v {
        "On"
    } else {
        "Off"
    }
}

fn button(id: RowId, label: &str, enabled: bool) -> Row {
    Row { id, label: label.to_string(), value: String::new(), kind: RowKind::Button, enabled }
}

fn choice(id: RowId, label: &str, value: &str) -> Row {
    Row { id, label: label.to_string(), value: format!("< {value} >"), kind: RowKind::Choice, enabled: true }
}

fn slider(id: RowId, label: &str, fraction: f32, text: String) -> Row {
    let n = (fraction.clamp(0.0, 1.0) * 10.0).round() as usize;
    Row { id, label: label.to_string(), value: format!("< {}{} {text} >", "#".repeat(n), "-".repeat(10 - n)), kind: RowKind::Slider, enabled: true }
}

fn volume(id: RowId, label: &str, v: f32) -> Row {
    slider(id, label, v, format!("{:>3.0}%", v * 100.0))
}

pub fn slot_name(slot: usize) -> String {
    match slot {
        AUTOSAVE => "Autosave".to_string(),
        QUICKSAVE => "Quicksave".to_string(),
        n => format!("Slot {}", n - FIRST_MANUAL + 1),
    }
}

impl Menu {
    /// The menu as it first appears when the game starts.
    pub fn title_screen() -> Menu {
        Menu { screen: Screen::Title, selected: 0, from_title: true }
    }

    pub fn title(&self) -> &'static str {
        match self.screen {
            Screen::Title => "THE LONG WINTER",
            Screen::Main => "PAUSED",
            Screen::Settings => "SETTINGS",
            Screen::Graphics => "GRAPHICS",
            Screen::Sound => "SOUND",
            Screen::Controls => "CONTROLS",
            Screen::Keys => "KEY BINDINGS",
            Screen::Save => "SAVE GAME",
            Screen::Load => "LOAD GAME",
            Screen::ConfirmQuit => "QUIT TO DESKTOP?",
        }
    }

    /// A line of explanation under the title, if the screen has one.
    pub fn note(&self) -> &'static str {
        match self.screen {
            Screen::ConfirmQuit if self.from_title => "",
            Screen::ConfirmQuit => "Anything since your last save will be lost.",
            Screen::Keys => "ENTER, then press the new key (ESC cancels). A key already in use swaps over.",
            Screen::Graphics => "Volumetric fog and ambient occlusion cost the most; turn them off first if it's slow.",
            _ => "",
        }
    }

    /// The screen to go back to from the top level.
    fn home(&self) -> Screen {
        if self.from_title {
            Screen::Title
        } else {
            Screen::Main
        }
    }

    pub fn rows(&self, s: &Settings, ctx: &Context) -> Vec<Row> {
        use RowId as R;
        match self.screen {
            Screen::Title => vec![
                button(R::Continue, "Continue", ctx.latest.is_some()),
                button(R::NewGame, "New Game", true),
                button(R::LoadGame, "Load Game", ctx.any_save()),
                button(R::Settings, "Settings", true),
                button(R::Quit, "Quit to Desktop", true),
            ],
            Screen::Main => vec![
                button(R::Resume, "Resume Game", true),
                button(R::SaveGame, "Save Game", ctx.alive),
                button(R::LoadGame, "Load Game", ctx.any_save()),
                button(R::Settings, "Settings", true),
                button(R::Quit, "Quit to Desktop", true),
            ],
            Screen::Settings => vec![button(R::Graphics, "Graphics", true), button(R::Sound, "Sound", true), button(R::Controls, "Controls", true), button(R::Back, "Back", true)],
            Screen::Graphics => vec![
                choice(R::Shadows, "Shadow Quality", s.shadows.label()),
                choice(R::View, "View Distance", s.view.label()),
                choice(R::Volumetrics, "Volumetric Fog", on_off(s.volumetrics)),
                choice(R::AmbientOcclusion, "Ambient Occlusion", on_off(s.ambient_occlusion)),
                choice(R::ShowFps, "Show Frame Rate", on_off(s.show_fps)),
                slider(R::UiScale, "UI Size", (s.ui_scale - UI_SCALE_MIN) / (UI_SCALE_MAX - UI_SCALE_MIN), format!("{:>3.0}%", s.ui_scale * 100.0)),
                button(R::Back, "Back", true),
            ],
            Screen::Sound => vec![
                volume(R::Master, "Master Volume", s.master),
                volume(R::Sfx, "Effects Volume", s.sfx),
                volume(R::Music, "Music Volume", s.music),
                volume(R::Ambience, "Ambience Volume", s.ambience),
                button(R::Back, "Back", true),
            ],
            Screen::Controls => vec![
                slider(
                    R::Sensitivity,
                    "Mouse Sensitivity",
                    (s.mouse_sensitivity - SENSITIVITY_MIN) / (SENSITIVITY_MAX - SENSITIVITY_MIN),
                    format!("{:.1}x", s.mouse_sensitivity),
                ),
                choice(R::InvertY, "Invert Mouse", on_off(s.invert_y)),
                slider(R::Fov, "Field of View", (s.fov - FOV_MIN) / (FOV_MAX - FOV_MIN), format!("{:>3.0}", s.fov)),
                button(R::Keys, "Key Bindings", true),
                button(R::Back, "Back", true),
            ],
            Screen::Keys => {
                let mut rows: Vec<Row> = Bind::ALL
                    .into_iter()
                    .map(|b| Row { id: R::Bind(b), label: b.label().to_string(), value: keys::display(s.keys.key(b)), kind: RowKind::Button, enabled: true })
                    .collect();
                rows.push(button(R::ResetKeys, "Reset to Defaults", s.keys != Bindings::default()));
                rows.push(button(R::Back, "Back", true));
                rows
            }
            Screen::Save => {
                let mut rows: Vec<Row> = (FIRST_MANUAL..SLOTS)
                    .map(|i| Row {
                        id: R::Slot(i),
                        label: slot_name(i),
                        value: ctx.slots[i].clone().unwrap_or_else(|| "- empty -".to_string()),
                        kind: RowKind::Button,
                        enabled: ctx.alive,
                    })
                    .collect();
                rows.push(button(R::Back, "Back", true));
                rows
            }
            Screen::Load => {
                let mut rows: Vec<Row> = (0..SLOTS)
                    .map(|i| Row {
                        id: R::Slot(i),
                        label: slot_name(i),
                        value: ctx.slots[i].clone().unwrap_or_else(|| "- empty -".to_string()),
                        kind: RowKind::Button,
                        enabled: ctx.slots[i].is_some(),
                    })
                    .collect();
                rows.push(button(R::Back, "Back", true));
                rows
            }
            Screen::ConfirmQuit => vec![button(R::No, if self.from_title { "No" } else { "No, keep playing" }, true), button(R::Yes, "Yes, quit", true)],
        }
    }

    /// The slice of a screen's `len` rows to draw so the selection is visible.
    pub fn window(&self, len: usize) -> (usize, usize) {
        let shown = VISIBLE_ROWS.min(len);
        let start = self.selected.saturating_sub(VISIBLE_ROWS / 2).min(len - shown);
        (start, start + shown)
    }

    fn go(&mut self, screen: Screen, selected: usize) {
        self.screen = screen;
        self.selected = selected;
    }

    /// If the selected row can't be chosen (Continue with no saves), move to
    /// the first one that can.
    pub fn ensure_enabled(&mut self, rows: &[Row]) {
        if !rows.get(self.selected).is_some_and(|r| r.enabled) {
            self.select_first_enabled(rows);
        }
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

    /// Esc: back out of a sub-screen, or close the pause menu. On the title
    /// screen there's nowhere to go back to.
    pub fn back(&mut self) -> Action {
        let home = self.home();
        match self.screen {
            Screen::Title => {}
            Screen::Main => return Action::Resume,
            Screen::Settings => self.go(home, 3),
            Screen::Graphics => self.go(Screen::Settings, 0),
            Screen::Sound => self.go(Screen::Settings, 1),
            Screen::Controls => self.go(Screen::Settings, 2),
            Screen::Keys => self.go(Screen::Controls, 3),
            Screen::Save => self.go(home, 1),
            Screen::Load => self.go(home, 2),
            Screen::ConfirmQuit => self.go(home, 4),
        }
        Action::None
    }

    /// Enter on the selected row. May change `settings` (a Choice or Slider
    /// row steps on, wrapping Choices round to the first value).
    pub fn activate(&mut self, settings: &mut Settings, ctx: &Context) -> (Action, bool) {
        let rows = self.rows(settings, ctx);
        let Some(row) = rows.get(self.selected) else { return (Action::None, false) };
        if !row.enabled {
            return (Action::None, false);
        }
        let open = |m: &mut Menu, screen: Screen, settings: &Settings| {
            m.go(screen, 0);
            let rows = m.rows(settings, ctx);
            m.select_first_enabled(&rows);
        };
        match row.id {
            RowId::Resume => return (Action::Resume, false),
            RowId::Continue => return (ctx.latest.map_or(Action::None, Action::Load), false),
            RowId::NewGame => return (Action::NewGame, false),
            RowId::SaveGame => open(self, Screen::Save, settings),
            RowId::LoadGame => open(self, Screen::Load, settings),
            RowId::Settings => self.go(Screen::Settings, 0),
            RowId::Graphics => self.go(Screen::Graphics, 0),
            RowId::Sound => self.go(Screen::Sound, 0),
            RowId::Controls => self.go(Screen::Controls, 0),
            RowId::Keys => self.go(Screen::Keys, 0),
            RowId::Quit => self.go(Screen::ConfirmQuit, 0),
            RowId::Back | RowId::No => {
                self.back();
            }
            RowId::Yes => return (Action::Quit, false),
            RowId::Slot(i) => return (if self.screen == Screen::Save { Action::Save(i) } else { Action::Load(i) }, false),
            RowId::Bind(b) => return (Action::Rebind(b), false),
            RowId::ResetKeys => {
                settings.keys.reset();
                return (Action::None, true);
            }
            _ => return (Action::None, self.change(1, true, settings)),
        }
        (Action::None, false)
    }

    /// Left (`-1`) or right (`+1`) on a Choice or Slider row. Returns true if
    /// a setting changed.
    pub fn adjust(&mut self, dir: i32, settings: &mut Settings, ctx: &Context) -> bool {
        let rows = self.rows(settings, ctx);
        match rows.get(self.selected) {
            Some(r) if r.kind != RowKind::Button => self.change(dir, false, settings),
            _ => false,
        }
    }

    /// Put a key on an action (after the menu asked for one). Returns true if
    /// anything changed.
    pub fn rebind(&mut self, bind: Bind, key: &str, settings: &mut Settings) -> bool {
        settings.keys.assign(bind, key)
    }

    /// `dir` is the step; with `wrap`, Enter cycling past the last choice
    /// returns to the first (and a slider at its top goes back to the bottom).
    fn change(&mut self, dir: i32, wrap: bool, s: &mut Settings) -> bool {
        let id = match self.rows(s, &Context { alive: true, slots: Default::default(), latest: None }).get(self.selected) {
            Some(r) => r.id,
            None => return false,
        };
        let before = *s;
        let tenth = |v: f32| if wrap && v >= 1.0 { 0.0 } else { (((v + dir as f32 * 0.1) * 10.0).round() / 10.0).clamp(0.0, 1.0) };
        match id {
            RowId::Shadows => s.shadows = if wrap && s.shadows == ShadowQuality::High { ShadowQuality::Off } else { s.shadows.step(dir) },
            RowId::View => s.view = if wrap && s.view == ViewDistance::Far { ViewDistance::Near } else { s.view.step(dir) },
            RowId::UiScale => {
                if wrap && s.ui_scale >= UI_SCALE_MAX {
                    s.ui_scale = UI_SCALE_MIN;
                } else {
                    s.nudge_ui_scale(dir);
                }
            }
            RowId::Master => s.master = tenth(s.master),
            RowId::Sfx => s.sfx = tenth(s.sfx),
            RowId::Music => s.music = tenth(s.music),
            RowId::Ambience => s.ambience = tenth(s.ambience),
            RowId::Volumetrics => s.volumetrics = !s.volumetrics,
            RowId::AmbientOcclusion => s.ambient_occlusion = !s.ambient_occlusion,
            RowId::ShowFps => s.show_fps = !s.show_fps,
            RowId::InvertY => s.invert_y = !s.invert_y,
            RowId::Sensitivity => {
                if wrap && s.mouse_sensitivity >= SENSITIVITY_MAX {
                    s.mouse_sensitivity = SENSITIVITY_MIN;
                } else {
                    s.nudge_sensitivity(dir);
                }
            }
            RowId::Fov => {
                if wrap && s.fov >= FOV_MAX {
                    s.fov = FOV_MIN;
                } else {
                    s.nudge_fov(dir);
                }
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
        Context { alive: true, slots: [None, None, None, None, None], latest: None }
    }

    fn with_saves() -> Context {
        Context { alive: true, slots: [Some("Autosave, day 2".into()), Some("Quick".into()), None, Some("Slot two".into()), None], latest: Some(3) }
    }

    fn rows(m: &Menu, c: &Context) -> Vec<Row> {
        m.rows(&Settings::default(), c)
    }

    fn labels(r: &[Row]) -> Vec<&str> {
        r.iter().map(|r| r.label.as_str()).collect()
    }

    /// Select the row with this id.
    fn at(m: &mut Menu, id: RowId, s: &Settings, c: &Context) {
        m.selected = m.rows(s, c).iter().position(|r| r.id == id).expect("row exists");
    }

    #[test]
    fn the_pause_menu_lists_the_five_options_in_order() {
        let r = rows(&Menu::default(), &ctx());
        assert_eq!(labels(&r), ["Resume Game", "Save Game", "Load Game", "Settings", "Quit to Desktop"]);
        assert_eq!(Menu::default().title(), "PAUSED");
    }

    #[test]
    fn the_title_screen_offers_continue_new_game_load_settings_quit() {
        let m = Menu::title_screen();
        let r = rows(&m, &ctx());
        assert_eq!(labels(&r), ["Continue", "New Game", "Load Game", "Settings", "Quit to Desktop"]);
        assert!(!r[0].enabled && !r[2].enabled, "nothing to continue or load yet");
        let mut fresh = Menu::title_screen();
        fresh.ensure_enabled(&r);
        assert_eq!(fresh.selected, 1, "New Game is highlighted when there's nothing to continue");
        let r = rows(&m, &with_saves());
        assert!(r[0].enabled && r[2].enabled);
    }

    #[test]
    fn continue_loads_the_latest_save_and_new_game_starts_one() {
        let mut s = Settings::default();
        let c = with_saves();
        let mut m = Menu::title_screen();
        assert_eq!(m.activate(&mut s, &c).0, Action::Load(3));
        m.selected = 1;
        assert_eq!(m.activate(&mut s, &c).0, Action::NewGame);
        assert_eq!(m.back(), Action::None, "Esc on the title screen does nothing");
        assert_eq!(m.screen, Screen::Title);
    }

    #[test]
    fn sub_screens_opened_from_the_title_go_back_to_it() {
        let mut s = Settings::default();
        let c = with_saves();
        let mut m = Menu::title_screen();
        m.selected = 3;
        m.activate(&mut s, &c);
        assert_eq!(m.screen, Screen::Settings);
        m.back();
        assert_eq!((m.screen, m.selected), (Screen::Title, 3));
        m.selected = 4;
        m.activate(&mut s, &c);
        assert_eq!(m.screen, Screen::ConfirmQuit);
        assert!(m.note().is_empty(), "nothing to lose before you've played");
        m.activate(&mut s, &c);
        assert_eq!((m.screen, m.selected), (Screen::Title, 4), "No returns to the title");
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
        let c = ctx();
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
        let rows = vec![button(RowId::Back, "x", false)];
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
    }

    #[test]
    fn escape_closes_the_pause_menu_and_backs_out_of_the_others() {
        let mut s = Settings::default();
        let c = with_saves();
        let mut m = Menu::default();
        assert_eq!(m.back(), Action::Resume);
        for (row, screen) in [(1, Screen::Save), (2, Screen::Load), (3, Screen::Settings), (4, Screen::ConfirmQuit)] {
            let mut m = Menu { selected: row, ..Menu::default() };
            assert_eq!(m.activate(&mut s, &c), (Action::None, false));
            assert_eq!(m.screen, screen);
            assert_eq!(m.back(), Action::None);
            assert_eq!((m.screen, m.selected), (Screen::Main, row), "returns to the row you came from");
        }
    }

    #[test]
    fn settings_is_a_hub_of_graphics_sound_and_controls() {
        let mut s = Settings::default();
        let c = ctx();
        let mut m = Menu { screen: Screen::Settings, ..Menu::default() };
        assert_eq!(labels(&rows(&m, &c)), ["Graphics", "Sound", "Controls", "Back"]);
        for (row, screen) in [(0, Screen::Graphics), (1, Screen::Sound), (2, Screen::Controls)] {
            m.selected = row;
            m.activate(&mut s, &c);
            assert_eq!(m.screen, screen);
            m.back();
            assert_eq!((m.screen, m.selected), (Screen::Settings, row));
        }
        m.selected = 2;
        m.activate(&mut s, &c);
        at(&mut m, RowId::Keys, &s, &c);
        m.activate(&mut s, &c);
        assert_eq!(m.screen, Screen::Keys);
        m.back();
        assert_eq!((m.screen, m.selected), (Screen::Controls, 3));
    }

    #[test]
    fn the_save_screen_offers_three_manual_slots() {
        let mut s = Settings::default();
        let c = with_saves();
        let mut m = Menu { screen: Screen::Save, ..Menu::default() };
        let r = m.rows(&s, &c);
        assert_eq!(r.len(), 4, "three slots and Back");
        assert_eq!((r[0].label.as_str(), r[0].value.as_str()), ("Slot 1", "- empty -"));
        assert_eq!(m.activate(&mut s, &c).0, Action::Save(FIRST_MANUAL));
        m.selected = 2;
        assert_eq!(m.activate(&mut s, &c).0, Action::Save(SLOTS - 1));
        m.selected = 3;
        m.activate(&mut s, &c);
        assert_eq!(m.screen, Screen::Main, "Back");
        let dead = Context { alive: false, ..c };
        assert!(!Menu { screen: Screen::Save, ..Menu::default() }.rows(&s, &dead)[0].enabled);
    }

    #[test]
    fn the_load_screen_includes_autosave_and_quicksave_and_only_filled_slots_load() {
        let mut s = Settings::default();
        let c = with_saves();
        let r = Menu { screen: Screen::Load, ..Menu::default() }.rows(&s, &c);
        assert_eq!(r.len(), SLOTS + 1);
        assert_eq!(r.iter().map(|r| r.enabled).collect::<Vec<_>>(), [true, true, false, true, false, true]);
        let mut m = Menu { screen: Screen::Load, selected: 3, ..Menu::default() };
        assert_eq!(m.activate(&mut s, &c).0, Action::Load(3));
        let mut m = Menu { screen: Screen::Load, selected: 2, ..Menu::default() };
        assert_eq!(m.activate(&mut s, &c), (Action::None, false), "an empty slot does nothing");
    }

    #[test]
    fn slots_are_named_for_what_they_are() {
        assert_eq!(slot_name(AUTOSAVE), "Autosave");
        assert_eq!(slot_name(QUICKSAVE), "Quicksave");
        assert_eq!((slot_name(FIRST_MANUAL), slot_name(SLOTS - 1)), ("Slot 1".to_string(), "Slot 3".to_string()));
    }

    #[test]
    fn quitting_asks_first_and_defaults_to_no() {
        let mut s = Settings::default();
        let c = ctx();
        let mut m = Menu { selected: 4, ..Menu::default() };
        m.activate(&mut s, &c);
        assert_eq!((m.screen, m.selected), (Screen::ConfirmQuit, 0), "No is highlighted");
        assert!(!m.note().is_empty());
        m.activate(&mut s, &c);
        assert_eq!(m.screen, Screen::Main);
        m.selected = 4;
        m.activate(&mut s, &c);
        m.selected = 1;
        assert_eq!(m.activate(&mut s, &c).0, Action::Quit);
    }

    #[test]
    fn left_and_right_change_the_matching_setting_and_nothing_else() {
        let base = Settings::default();
        let c = ctx();
        for (screen, id, check) in [
            (Screen::Graphics, RowId::Shadows, (|a: &Settings, b: &Settings| a.shadows != b.shadows) as fn(&Settings, &Settings) -> bool),
            (Screen::Graphics, RowId::View, |a, b| a.view != b.view),
            (Screen::Graphics, RowId::UiScale, |a, b| a.ui_scale != b.ui_scale),
            (Screen::Sound, RowId::Master, |a, b| a.master != b.master),
            (Screen::Sound, RowId::Sfx, |a, b| a.sfx != b.sfx),
            (Screen::Sound, RowId::Music, |a, b| a.music != b.music),
            (Screen::Sound, RowId::Ambience, |a, b| a.ambience != b.ambience),
            (Screen::Controls, RowId::Sensitivity, |a, b| a.mouse_sensitivity != b.mouse_sensitivity),
            (Screen::Controls, RowId::Fov, |a, b| a.fov != b.fov),
        ] {
            let mut m = Menu { screen, ..Menu::default() };
            let mut s = base;
            at(&mut m, id, &s, &c);
            assert!(m.adjust(-1, &mut s, &c), "{id:?}");
            assert!(check(&s, &base), "{id:?} changed");
            // Exactly one field differs: put it back and the rest must match.
            let mut fixed = s;
            match id {
                RowId::Shadows => fixed.shadows = base.shadows,
                RowId::View => fixed.view = base.view,
                RowId::UiScale => fixed.ui_scale = base.ui_scale,
                RowId::Master => fixed.master = base.master,
                RowId::Sfx => fixed.sfx = base.sfx,
                RowId::Music => fixed.music = base.music,
                RowId::Ambience => fixed.ambience = base.ambience,
                RowId::Sensitivity => fixed.mouse_sensitivity = base.mouse_sensitivity,
                _ => fixed.fov = base.fov,
            }
            assert_eq!(fixed, base, "{id:?} touched something else");
        }
    }

    #[test]
    fn toggles_flip_and_buttons_do_not_adjust() {
        let c = ctx();
        let mut s = Settings::default();
        for (screen, id) in [(Screen::Graphics, RowId::Volumetrics), (Screen::Graphics, RowId::AmbientOcclusion), (Screen::Graphics, RowId::ShowFps), (Screen::Controls, RowId::InvertY)] {
            let mut m = Menu { screen, ..Menu::default() };
            at(&mut m, id, &s, &c);
            let before = s;
            assert!(m.adjust(1, &mut s, &c));
            assert_ne!(s, before);
            assert!(m.adjust(1, &mut s, &c));
            assert_eq!(s, before, "{id:?} flips back");
        }
        let mut m = Menu { screen: Screen::Graphics, ..Menu::default() };
        at(&mut m, RowId::Back, &s, &c);
        assert!(!m.adjust(1, &mut s, &c), "Back isn't a setting");
        assert!(!Menu::default().adjust(1, &mut s, &c), "nor are the pause menu's buttons");
    }

    #[test]
    fn arrows_stop_at_the_ends_but_enter_cycles_round() {
        let c = ctx();
        let mut s = Settings::default();
        let mut m = Menu { screen: Screen::Graphics, ..Menu::default() };
        assert!(!m.adjust(1, &mut s, &c), "High is already the top");
        assert_eq!(m.activate(&mut s, &c), (Action::None, true));
        assert_eq!(s.shadows, ShadowQuality::Off, "Enter wraps High to Off");
        let mut m = Menu { screen: Screen::Sound, ..Menu::default() };
        s.master = 1.0;
        assert!(!m.adjust(1, &mut s, &c), "slider stops at 100%");
        m.activate(&mut s, &c);
        assert_eq!(s.master, 0.0);
        for _ in 0..3 {
            m.adjust(1, &mut s, &c);
        }
        assert_eq!(s.master, 0.3, "clean tenths");
        assert!(m.rows(&s, &c)[0].value.contains("###-------"));
    }

    #[test]
    fn controls_show_their_values() {
        let s = Settings { mouse_sensitivity: 1.5, fov: 90.0, invert_y: true, ..Settings::default() };
        let r = Menu { screen: Screen::Controls, ..Menu::default() }.rows(&s, &ctx());
        assert_eq!(labels(&r), ["Mouse Sensitivity", "Invert Mouse", "Field of View", "Key Bindings", "Back"]);
        assert!(r[0].value.contains("1.5x"));
        assert_eq!(r[1].value, "< On >");
        assert!(r[2].value.contains("90"));
    }

    #[test]
    fn the_key_screen_lists_every_action_and_rebinds_on_request() {
        let c = ctx();
        let mut s = Settings::default();
        let mut m = Menu { screen: Screen::Keys, ..Menu::default() };
        let r = m.rows(&s, &c);
        assert_eq!(r.len(), Bind::COUNT + 2);
        assert_eq!((r[0].label.as_str(), r[0].value.as_str()), ("Move Forward", "W"));
        assert!(!r[Bind::COUNT].enabled, "nothing to reset yet");
        assert_eq!(m.activate(&mut s, &c).0, Action::Rebind(Bind::Forward));
        assert!(m.rebind(Bind::Forward, "ArrowUp", &mut s));
        assert_eq!(s.keys.key(Bind::Forward), "ArrowUp");
        m.selected = Bind::COUNT;
        assert_eq!(m.activate(&mut s, &c), (Action::None, true), "Reset to Defaults");
        assert_eq!(s.keys, Bindings::default());
    }

    #[test]
    fn long_screens_scroll_to_keep_the_selection_in_view() {
        let mut m = Menu { screen: Screen::Keys, ..Menu::default() };
        let n = Bind::COUNT + 2;
        for sel in 0..n {
            m.selected = sel;
            let (a, b) = m.window(n);
            assert!((a..b).contains(&sel) && b - a == VISIBLE_ROWS.min(n) && b <= n, "{sel}: {a}..{b}");
        }
        m.selected = 0;
        assert_eq!(m.window(4), (0, 4), "short screens show whole");
    }

    #[test]
    fn activating_a_disabled_row_is_ignored() {
        let mut s = Settings::default();
        let mut m = Menu { selected: 2, ..Menu::default() };
        assert_eq!(m.activate(&mut s, &ctx()), (Action::None, false));
        assert_eq!(m.screen, Screen::Main);
    }
}
