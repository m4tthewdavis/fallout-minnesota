//! How you move around the Pip-Boy: three top tabs (STATS, ITEMS, DATA), each
//! with pages along the bottom, and a selected row on each list page. Pure
//! rules so the navigation can be unit-tested; `pipboy.rs` draws it.

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Main {
    Stats,
    Items,
    Data,
}

impl Main {
    pub const ALL: [Main; 3] = [Main::Stats, Main::Items, Main::Data];

    pub fn label(self) -> &'static str {
        match self {
            Main::Stats => "STATS",
            Main::Items => "ITEMS",
            Main::Data => "DATA",
        }
    }

    pub fn pages(self) -> &'static [Page] {
        match self {
            Main::Stats => &[Page::Status, Page::Special],
            Main::Items => &[Page::Weapons, Page::Apparel, Page::Aid],
            Main::Data => &[Page::Map, Page::Notes],
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Page {
    Status,
    Special,
    Weapons,
    Apparel,
    Aid,
    Map,
    Notes,
}

impl Page {
    #[cfg(test)]
    pub const ALL: [Page; 7] = [Page::Status, Page::Special, Page::Weapons, Page::Apparel, Page::Aid, Page::Map, Page::Notes];

    pub fn main(self) -> Main {
        match self {
            Page::Status | Page::Special => Main::Stats,
            Page::Weapons | Page::Apparel | Page::Aid => Main::Items,
            Page::Map | Page::Notes => Main::Data,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Page::Status => "STATUS",
            Page::Special => "S.P.E.C.I.A.L.",
            Page::Weapons => "WEAPONS",
            Page::Apparel => "APPAREL",
            Page::Aid => "AID",
            Page::Map => "WORLD MAP",
            Page::Notes => "NOTES",
        }
    }

    pub fn index(self) -> usize {
        self as usize
    }

    /// Which of the shared page layouts shows this page.
    pub fn layout(self) -> Layout {
        match self {
            Page::Status => Layout::Status,
            Page::Special | Page::Weapons | Page::Apparel | Page::Aid => Layout::List,
            Page::Map => Layout::Map,
            Page::Notes => Layout::Notes,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Layout {
    Status,
    List,
    Map,
    Notes,
}

/// Where you are in the Pip-Boy.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Nav {
    pub page: Page,
    /// The page each top tab last showed, so tabs reopen where you left them.
    pub last: [Page; 3],
    /// Selected row on each page (only list pages use theirs).
    pub selected: [usize; 7],
}

impl Default for Nav {
    fn default() -> Self {
        Nav { page: Page::Status, last: [Page::Status, Page::Weapons, Page::Map], selected: [0; 7] }
    }
}

impl Nav {
    /// Jump to a page. Returns true if that changed anything.
    pub fn go_to(&mut self, page: Page) -> bool {
        if self.page == page {
            return false;
        }
        self.page = page;
        self.last[page.main() as usize] = page;
        true
    }

    /// Switch top tab, landing on the page it last showed.
    pub fn switch_main(&mut self, main: Main) -> bool {
        if self.page.main() == main {
            return false;
        }
        self.go_to(self.last[main as usize])
    }

    /// One page left (`-1`) or right (`+1`) within this tab; no wrap-around.
    pub fn step_page(&mut self, dir: i32) -> bool {
        let pages = self.page.main().pages();
        let at = pages.iter().position(|p| *p == self.page).unwrap_or(0) as i32;
        let to = at + dir;
        if to < 0 || to >= pages.len() as i32 {
            return false;
        }
        self.go_to(pages[to as usize])
    }

    /// The `n`th page of the current tab (for clicking the page labels).
    pub fn page_in_tab(&self, n: usize) -> Option<Page> {
        self.page.main().pages().get(n).copied()
    }

    pub fn selected_row(&self) -> usize {
        self.selected[self.page.index()]
    }

    /// Move the selection up (`-1`) or down (`+1`) a list of `count` rows,
    /// stopping at the ends. Returns true if it moved.
    pub fn move_selection(&mut self, count: usize, dir: i32) -> bool {
        let i = self.page.index();
        let max = count.max(1) - 1;
        let before = self.selected[i].min(max);
        let after = (before as i32 + dir).clamp(0, max as i32) as usize;
        self.selected[i] = after;
        after != before
    }

    /// Select a row directly (the mouse hovering it). Returns true if it changed.
    pub fn select_row(&mut self, row: usize, count: usize) -> bool {
        let i = self.page.index();
        let row = row.min(count.max(1) - 1);
        let changed = self.selected[i] != row;
        self.selected[i] = row;
        changed
    }

    /// Keep the selection valid after a list gets shorter.
    pub fn clamp_selection(&mut self, count: usize) {
        let i = self.page.index();
        self.selected[i] = self.selected[i].min(count.max(1) - 1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_page_belongs_to_exactly_one_tab_in_order() {
        for page in Page::ALL {
            let tabs: Vec<_> = Main::ALL.iter().filter(|m| m.pages().contains(&page)).collect();
            assert_eq!(tabs, vec![&page.main()], "{page:?}");
        }
        let total: usize = Main::ALL.iter().map(|m| m.pages().len()).sum();
        assert_eq!(total, Page::ALL.len());
        for (i, page) in Page::ALL.into_iter().enumerate() {
            assert_eq!(page.index(), i, "selection slots line up with the pages");
        }
    }

    #[test]
    fn tabs_reopen_on_the_page_you_left() {
        let mut nav = Nav::default();
        assert_eq!(nav.page, Page::Status);
        assert!(nav.switch_main(Main::Items));
        assert_eq!(nav.page, Page::Weapons, "first visit lands on the first page");
        assert!(nav.step_page(1) && nav.step_page(1));
        assert_eq!(nav.page, Page::Aid);
        nav.switch_main(Main::Data);
        nav.switch_main(Main::Items);
        assert_eq!(nav.page, Page::Aid, "remembers AID");
        assert!(!nav.switch_main(Main::Items), "already there");
    }

    #[test]
    fn paging_stops_at_both_ends_of_a_tab() {
        let mut nav = Nav::default();
        assert!(!nav.step_page(-1), "STATUS is the first page");
        assert!(nav.step_page(1));
        assert_eq!(nav.page, Page::Special);
        assert!(!nav.step_page(1), "S.P.E.C.I.A.L. is the last page of STATS");
        assert_eq!(nav.page, Page::Special);
        assert!(nav.step_page(-1));
        assert_eq!(nav.page, Page::Status);
    }

    #[test]
    fn clicking_a_page_label_goes_there_and_extra_labels_do_nothing() {
        let mut nav = Nav::default();
        nav.switch_main(Main::Data);
        assert_eq!(nav.page_in_tab(1), Some(Page::Notes));
        assert_eq!(nav.page_in_tab(2), None, "DATA has only two pages");
        assert!(nav.go_to(Page::Notes));
        assert!(!nav.go_to(Page::Notes));
        assert_eq!(nav.last[Main::Data as usize], Page::Notes);
    }

    #[test]
    fn list_selection_moves_one_row_and_stops_at_the_ends() {
        let mut nav = Nav::default();
        nav.go_to(Page::Aid);
        assert!(!nav.move_selection(3, -1), "already at the top");
        assert!(nav.move_selection(3, 1) && nav.move_selection(3, 1));
        assert_eq!(nav.selected_row(), 2);
        assert!(!nav.move_selection(3, 1), "already at the bottom");
        assert!(nav.move_selection(3, -1));
        assert_eq!(nav.selected_row(), 1);
    }

    #[test]
    fn each_list_page_remembers_its_own_selection() {
        let mut nav = Nav::default();
        nav.go_to(Page::Weapons);
        nav.move_selection(4, 1);
        nav.go_to(Page::Aid);
        assert_eq!(nav.selected_row(), 0);
        nav.move_selection(3, 1);
        nav.go_to(Page::Weapons);
        assert_eq!(nav.selected_row(), 1, "weapons kept its row");
    }

    #[test]
    fn the_selection_survives_a_shrinking_list_and_an_empty_one() {
        let mut nav = Nav::default();
        nav.go_to(Page::Weapons);
        nav.select_row(3, 4);
        assert_eq!(nav.selected_row(), 3);
        nav.clamp_selection(2);
        assert_eq!(nav.selected_row(), 1, "pulled back inside the list");
        nav.clamp_selection(0);
        assert_eq!(nav.selected_row(), 0, "an empty list selects row 0, never underflows");
        assert!(!nav.move_selection(0, 1));
        assert!(nav.select_row(9, 3));
        assert_eq!(nav.selected_row(), 2, "hovering past the end clamps");
        assert!(!nav.select_row(2, 3), "no change when it's already selected");
    }

    #[test]
    fn page_labels_and_layouts() {
        assert_eq!(Page::Special.label(), "S.P.E.C.I.A.L.");
        assert_eq!(Main::Data.label(), "DATA");
        assert_eq!(Page::Status.layout(), Layout::Status);
        assert_eq!(Page::Map.layout(), Layout::Map);
        for p in [Page::Special, Page::Weapons, Page::Apparel, Page::Aid] {
            assert_eq!(p.layout(), Layout::List, "{p:?}");
        }
    }
}
