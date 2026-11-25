//! The Swipe and select page: list rows with swipe actions, Select mode,
//! and an undo after every delete.
use super::*;
use std::collections::BTreeSet;

pub struct Note {
    pub id: u32,
    pub title: &'static str,
    pub preview: &'static str,
    pub age: &'static str,
}
const NOTES: [(&str, &str, &str); 8] = [
    ("Groceries", "Oat milk, lemons, rice", "9:41"),
    ("Release plan", "Cut the branch on Thursday", "Yesterday"),
    ("Books", "The Dispossessed, Piranesi", "Mon"),
    ("Trip", "Train at 7:10 from platform 4", "Sun"),
    ("Ideas", "A dock that rides the keyboard", "Sat"),
    ("Gifts", "Something for Sam", "Fri"),
    ("Recipes", "Dal with lots of ginger", "Thu"),
    ("Running", "5 km in 27 minutes", "Wed"),
];

pub struct Rows {
    pub notes: Vec<Note>,
    pub open: Option<(u32, SwipeSide)>,
    pub selecting: bool,
    pub selected: BTreeSet<u32>,
    pub pinned: BTreeSet<u32>,
    pub flagged: BTreeSet<u32>,
    /// The last delete, for Undo: the notes and where they were.
    pub deleted: Vec<(usize, Note)>,
    pub more: bool,
    /// The note whose long-press menu is open.
    pub menu: Option<u32>,
    /// The first row has nudged to show it swipes; once is enough.
    pub hinted: bool,
    /// Bumped by each delete; the Undo bar's timer only closes its own.
    pub undo_round: u64,
}
impl Default for Rows {
    fn default() -> Self {
        Self {
            notes: NOTES
                .iter()
                .enumerate()
                .map(|(i, (title, preview, age))| Note {
                    id: i as u32,
                    title,
                    preview,
                    age,
                })
                .collect(),
            open: None,
            selecting: false,
            selected: BTreeSet::new(),
            pinned: BTreeSet::new(),
            flagged: BTreeSet::new(),
            deleted: Vec::new(),
            more: false,
            menu: None,
            hinted: false,
            undo_round: 0,
        }
    }
}
impl Rows {
    fn delete(&mut self, ids: &BTreeSet<u32>) {
        self.undo_round += 1;
        self.deleted.clear();
        let mut i = 0;
        while i < self.notes.len() {
            if ids.contains(&self.notes[i].id) {
                let note = self.notes.remove(i);
                self.deleted.push((i + self.deleted.len(), note));
            } else {
                i += 1;
            }
        }
        self.open = None;
        self.selected.clear();
    }
    fn undo(&mut self) {
        for (at, note) in std::mem::take(&mut self.deleted) {
            let at = at.min(self.notes.len());
            self.notes.insert(at, note);
        }
    }
}

pub fn page(app: &App) -> Container<App> {
    let r = &app.rows;
    let rows: Vec<Element<App>> = r
        .notes
        .iter()
        .enumerate()
        .map(|(i, note)| {
            let id = note.id;
            let pinned = r.pinned.contains(&id);
            let flagged = r.flagged.contains(&id);
            let title = if flagged {
                format!("{} (flagged)", note.title)
            } else {
                note.title.to_owned()
            };
            list_row(title)
                .key(format!("note-{id}"))
                .subtitle(note.preview)
                .value(note.age)
                .icon(
                    if pinned { Icon::Star } else { Icon::File },
                    if pinned {
                        ColorToken::Warning
                    } else {
                        ColorToken::Info
                    },
                )
                .chevron(true)
                .on_press(|_: &mut App| {})
                .leading_actions([
                    swipe_action(if pinned { "Unpin" } else { "Pin" }, Icon::Star)
                        .tone(SwipeTone::Accent)
                        .full_swipe()
                        .on_press(move |a: &mut App| {
                            if !a.rows.pinned.remove(&id) {
                                a.rows.pinned.insert(id);
                            }
                        }),
                ])
                .trailing_actions([
                    swipe_action("Delete", Icon::Trash)
                        .tone(SwipeTone::Danger)
                        .full_swipe()
                        .on_press(move |a: &mut App| delete(a, &BTreeSet::from([id]))),
                    swipe_action(if flagged { "Unflag" } else { "Flag" }, Icon::Bell)
                        .tone(SwipeTone::Warning)
                        .on_press(move |a: &mut App| {
                            if !a.rows.flagged.remove(&id) {
                                a.rows.flagged.insert(id);
                            }
                        }),
                ])
                .swipe_hint(i == 0 && !r.hinted)
                .swipe_open(r.open.filter(|(o, _)| *o == id).map(|(_, side)| side))
                .on_swipe_open(move |a: &mut App, side| {
                    a.rows.open = side.map(|s| (id, s));
                    a.rows.hinted = true;
                })
                .hold_actions([
                    swipe_action(if pinned { "Unpin" } else { "Pin" }, Icon::Star).on_press(
                        move |a: &mut App| {
                            if !a.rows.pinned.remove(&id) {
                                a.rows.pinned.insert(id);
                            }
                        },
                    ),
                    swipe_action(if flagged { "Unflag" } else { "Flag" }, Icon::Bell).on_press(
                        move |a: &mut App| {
                            if !a.rows.flagged.remove(&id) {
                                a.rows.flagged.insert(id);
                            }
                        },
                    ),
                    swipe_action("Select", Icon::Check).on_press(move |a: &mut App| {
                        a.rows.selecting = true;
                        a.rows.open = None;
                        a.rows.selected = BTreeSet::from([id]);
                    }),
                    swipe_action("Delete", Icon::Trash)
                        .tone(SwipeTone::Danger)
                        .on_press(move |a: &mut App| delete(a, &BTreeSet::from([id]))),
                ])
                .menu_open(r.menu == Some(id))
                .on_menu_open(move |a: &mut App, open| {
                    a.rows.open = None;
                    a.rows.menu = open.then_some(id);
                })
                .selecting(r.selecting)
                .selected(r.selected.contains(&id))
                .on_toggle(move |a: &mut App| {
                    if !a.rows.selected.remove(&id) {
                        a.rows.selected.insert(id);
                    }
                })
                .into()
        })
        .collect();
    let mut parts: Vec<Element<App>> = Vec::new();
    let header = if r.selecting {
        match r.selected.len() {
            0 => "Choose notes".to_owned(),
            1 => "1 selected".to_owned(),
            n => format!("{n} selected"),
        }
    } else {
        format!("{} notes", r.notes.len())
    };
    parts.push(
        list_section(
            header,
            rows,
            "Swipe right to pin, left to flag or delete. Swipe far left and pause before letting go to delete at once. Touch and hold a note for its menu.",
        )
        .into(),
    );
    let list = super::page("rows-list", parts);
    let what = match r.deleted.len() {
        0 => String::new(),
        1 => format!("Deleted {}.", r.deleted[0].1.title),
        n => format!("Deleted {n} notes."),
    };
    let notice = || {
        snackbar(what.clone())
            .key("rows-undo-bar")
            .action("Undo", |a: &mut App| a.rows.undo())
            .open(!r.deleted.is_empty())
            .on_close(|a: &mut App| a.rows.deleted.clear())
    };
    let dock = if r.selecting {
        let n = r.selected.len();
        let all = n == r.notes.len() && n > 0;
        action_dock(list)
            .key("rows-dock")
            .notice(notice())
            .primary(
                dock_action(
                    if n == 0 {
                        "Delete".to_owned()
                    } else {
                        format!("Delete {n}")
                    },
                    Icon::Trash,
                )
                .danger()
                .enabled(n > 0)
                .on_press(|a: &mut App| {
                    let ids = std::mem::take(&mut a.rows.selected);
                    delete(a, &ids);
                    a.rows.selecting = false;
                }),
            )
            .secondary(
                dock_action("Done", Icon::Close)
                    .show_label()
                    .on_press(|a: &mut App| {
                        a.rows.selecting = false;
                        a.rows.selected.clear();
                    }),
            )
            .secondary(
                dock_action(if all { "Deselect all" } else { "Select all" }, Icon::Check)
                    .show_label()
                    .on_press(move |a: &mut App| {
                        if all {
                            a.rows.selected.clear();
                        } else {
                            a.rows.selected = a.rows.notes.iter().map(|n| n.id).collect();
                        }
                    }),
            )
    } else {
        action_dock(list)
            .key("rows-dock")
            .notice(notice())
            .follows("rows-list")
            .primary(dock_action("New note", Icon::Plus).on_press(|_: &mut App| {}))
            .more([dock_action("Select", Icon::Check).on_press(|a: &mut App| {
                a.rows.selecting = true;
                a.rows.open = None;
            })])
            .more_open(r.more)
            .on_more(|a: &mut App, open| a.rows.more = open)
    };
    vstack![dock].key("rows-screen").flex(1.)
}

/// Delete, then close the Undo bar after five seconds unless another delete
/// replaced it.
pub fn delete(a: &mut App, ids: &BTreeSet<u32>) {
    a.rows.delete(ids);
    let round = a.rows.undo_round;
    if let Some(updates) = a.updates.clone() {
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_secs(5));
            updates.update(move |a: &mut App| {
                if a.rows.undo_round == round {
                    a.rows.deleted.clear();
                }
            });
        });
    }
}
