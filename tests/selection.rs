#![cfg(feature = "rust-ui")]

use volt_ui_kit::ui::prelude::*;

const PLAIN: SelectionModifiers = SelectionModifiers {
    toggle: false,
    extend: false,
};
const TOGGLE: SelectionModifiers = SelectionModifiers {
    toggle: true,
    extend: false,
};
const EXTEND: SelectionModifiers = SelectionModifiers {
    toggle: false,
    extend: true,
};
const ADD_RANGE: SelectionModifiers = SelectionModifiers {
    toggle: true,
    extend: true,
};

fn model() -> SelectionModel<&'static str> {
    let mut m = SelectionModel::new();
    m.reconcile(&["a", "b", "c", "d", "e"]).unwrap();
    m
}

fn selected(m: &SelectionModel<&'static str>) -> Vec<&'static str> {
    m.selected_in_order().copied().collect()
}

#[test]
fn new_and_cleared_selection_are_truly_empty() {
    let mut m: SelectionModel<&str> = SelectionModel::default();
    assert!(m.is_empty());
    assert_eq!(m.focused(), None);
    assert_eq!(m.anchor(), None);
    assert!(!m.navigate(SelectionMove::Next, PLAIN));
    m.reconcile(&["a"]).unwrap();
    m.select(&"a", PLAIN);
    m.clear();
    assert!(m.is_empty());
    assert_eq!(m.focused(), None);
    assert_eq!(m.anchor(), None);
    assert_eq!(m.items(), &["a"]);
}

#[test]
fn ordinary_selection_replaces_and_sets_focus_and_anchor() {
    let mut m = model();
    assert!(m.select(&"b", PLAIN));
    m.select(&"d", PLAIN);
    assert_eq!(selected(&m), ["d"]);
    assert_eq!(m.focused(), Some(&"d"));
    assert_eq!(m.anchor(), Some(&"d"));
}

#[test]
fn toggle_can_deselect_last_item_without_losing_keyboard_focus() {
    let mut m = model();
    m.select(&"a", TOGGLE);
    m.select(&"c", TOGGLE);
    m.select(&"a", TOGGLE);
    m.select(&"c", TOGGLE);
    assert!(m.is_empty());
    assert_eq!(m.focused(), Some(&"c"));
    assert_eq!(m.anchor(), Some(&"c"));
}

#[test]
fn shift_ranges_expand_contract_and_reverse_around_stable_anchor() {
    let mut m = model();
    m.select(&"c", PLAIN);
    m.select(&"e", EXTEND);
    assert_eq!(selected(&m), ["c", "d", "e"]);
    m.select(&"d", EXTEND);
    assert_eq!(selected(&m), ["c", "d"]);
    m.select(&"a", EXTEND);
    assert_eq!(selected(&m), ["a", "b", "c"]);
    assert_eq!(m.anchor(), Some(&"c"));
}

#[test]
fn toggle_shift_adds_range_without_discarding_other_members() {
    let mut m = model();
    m.select(&"a", TOGGLE);
    m.select(&"d", TOGGLE);
    m.select(&"e", ADD_RANGE);
    assert_eq!(selected(&m), ["a", "d", "e"]);
    assert_eq!(m.anchor(), Some(&"d"));
}

#[test]
fn first_shift_with_no_anchor_selects_target_only() {
    let mut m = model();
    m.select(&"d", EXTEND);
    assert_eq!(selected(&m), ["d"]);
    assert_eq!(m.anchor(), Some(&"d"));
}

#[test]
fn unknown_identity_does_not_clear_or_change_state() {
    let mut m = model();
    m.select(&"b", PLAIN);
    for modifiers in [PLAIN, TOGGLE, EXTEND, ADD_RANGE] {
        assert!(!m.select(&"missing", modifiers));
        assert_eq!(selected(&m), ["b"]);
        assert_eq!(m.anchor(), Some(&"b"));
        assert_eq!(m.focused(), Some(&"b"));
    }
}

#[test]
fn navigate_respects_boundaries_and_extends_keyboard_ranges() {
    let mut m = model();
    m.navigate(SelectionMove::Next, PLAIN);
    assert_eq!(selected(&m), ["a"]);
    m.navigate(SelectionMove::Previous, PLAIN);
    assert_eq!(selected(&m), ["a"]);
    m.navigate(SelectionMove::Next, EXTEND);
    m.navigate(SelectionMove::Next, EXTEND);
    assert_eq!(selected(&m), ["a", "b", "c"]);
    m.navigate(SelectionMove::Previous, EXTEND);
    assert_eq!(selected(&m), ["a", "b"]);
    m.navigate(SelectionMove::Last, PLAIN);
    m.navigate(SelectionMove::Next, PLAIN);
    assert_eq!(selected(&m), ["e"]);
    m.navigate(SelectionMove::First, EXTEND);
    assert_eq!(selected(&m), ["a", "b", "c", "d", "e"]);
}

#[test]
fn keyboard_toggle_modifier_moves_focus_without_toggling_selection() {
    let mut m = model();
    m.select(&"b", PLAIN);
    m.navigate(SelectionMove::Next, TOGGLE);
    assert_eq!(selected(&m), ["b"]);
    assert_eq!(m.focused(), Some(&"c"));
    assert_eq!(m.anchor(), Some(&"b"));
    m.navigate(SelectionMove::Next, EXTEND);
    assert_eq!(selected(&m), ["b", "c", "d"]);
}

#[test]
fn focus_only_on_empty_model_order_does_not_select() {
    let mut m = model();
    m.navigate(SelectionMove::Previous, TOGGLE);
    assert!(m.is_empty());
    assert_eq!(m.focused(), Some(&"e"));
    m.navigate(SelectionMove::Previous, EXTEND);
    assert_eq!(selected(&m), ["d", "e"]);
}

#[test]
fn reconcile_keeps_stable_members_and_anchor_through_sorting() {
    let mut m = model();
    m.select(&"b", PLAIN);
    m.select(&"d", EXTEND);
    m.reconcile(&["new", "d", "c", "b", "a"]).unwrap();
    assert_eq!(selected(&m), ["d", "c", "b"]);
    assert!(!m.contains(&"new"));
    assert_eq!(m.anchor(), Some(&"b"));
    m.select(&"a", EXTEND);
    assert_eq!(selected(&m), ["b", "a"]);
}

#[test]
fn reconcile_removes_absent_members_and_repairs_focus_and_anchor() {
    let mut m = model();
    m.select(&"b", PLAIN);
    m.select(&"d", EXTEND);
    m.reconcile(&["a", "c", "e"]).unwrap();
    assert_eq!(selected(&m), ["c"]);
    assert_eq!(m.focused(), Some(&"c"));
    assert_eq!(m.anchor(), Some(&"c"));
    m.reconcile(&["unseen"]).unwrap();
    assert!(m.is_empty());
    assert_eq!(m.focused(), None);
    assert_eq!(m.anchor(), None);
}

#[test]
fn empty_reconcile_clears_every_identity() {
    let mut m = model();
    m.select_all();
    m.reconcile(&[]).unwrap();
    assert!(m.items().is_empty());
    assert!(m.is_empty());
    assert_eq!(m.focused(), None);
    assert_eq!(m.anchor(), None);
}

#[test]
fn invalid_reconcile_is_atomic_and_rejects_duplicates() {
    let mut m = model();
    m.select(&"d", PLAIN);
    assert_eq!(
        m.reconcile(&["x", "x"]),
        Err(SelectionError::DuplicateIdentity { index: 1 })
    );
    assert_eq!(selected(&m), ["d"]);
    assert_eq!(m.items(), &["a", "b", "c", "d", "e"]);
    assert_eq!(m.focused(), Some(&"d"));
    assert_eq!(m.anchor(), Some(&"d"));
}

#[test]
fn select_all_uses_current_scope_only_and_reconcile_does_not_expand_it() {
    let mut m = model();
    m.select_all();
    assert_eq!(m.len(), 5);
    m.reconcile(&["a", "b", "new"]).unwrap();
    assert_eq!(selected(&m), ["a", "b"]);
    assert_eq!(m.len(), 2);
}

#[test]
fn maximum_order_is_accepted_and_larger_input_rejected_without_mutation() {
    let mut m = SelectionModel::new();
    let mut ids: Vec<usize> = (0..SelectionModel::<usize>::MAX_ITEMS).collect();
    m.reconcile(&ids).unwrap();
    m.select(&10, PLAIN);
    ids.push(SelectionModel::<usize>::MAX_ITEMS);
    assert_eq!(
        m.reconcile(&ids),
        Err(SelectionError::TooManyItems {
            count: ids.len(),
            maximum: SelectionModel::<usize>::MAX_ITEMS
        })
    );
    assert!(m.contains(&10));
    assert_eq!(m.items().len(), SelectionModel::<usize>::MAX_ITEMS);
}

#[test]
fn mixed_operations_preserve_identity_invariants_across_scope_changes() {
    let orders = [vec![0, 1, 2, 3, 4], vec![4, 2, 0, 1], vec![5, 3, 1], vec![]];
    let mut m = SelectionModel::new();
    let mut seed = 1_u32;
    for step in 0..10_000 {
        seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        let modifiers = [PLAIN, TOGGLE, EXTEND, ADD_RANGE][(seed as usize >> 8) % 4];
        match seed % 6 {
            0 => {
                // Selection may shrink during reconciliation, never grow.
                let before: Vec<_> = m.selected_in_order().copied().collect();
                m.reconcile(&orders[(seed as usize >> 12) % orders.len()])
                    .unwrap();
                assert!(m.selected_in_order().all(|id| before.contains(id)));
            }
            1 => {
                m.select(&(step % 7), modifiers);
            }
            2 | 3 => {
                let movement = [
                    SelectionMove::First,
                    SelectionMove::Previous,
                    SelectionMove::Next,
                    SelectionMove::Last,
                ][(seed as usize >> 16) % 4];
                m.navigate(movement, modifiers);
            }
            4 => m.select_all(),
            _ => m.clear(),
        }
        assert!(m.len() <= m.items().len());
        assert_eq!(m.selected_in_order().count(), m.len());
        assert_eq!(m.is_empty(), m.selected_in_order().next().is_none());
        assert!(m.focused().is_none_or(|id| m.items().contains(id)));
        assert!(m.anchor().is_none_or(|id| m.items().contains(id)));
        assert!(m.selected_in_order().all(|id| m.items().contains(id)));
    }
}

#[test]
fn cloning_does_not_share_mutable_selection_state() {
    let mut a = model();
    a.select(&"a", PLAIN);
    let mut b = a.clone();
    b.select(&"e", PLAIN);
    assert_eq!(selected(&a), ["a"]);
    assert_eq!(selected(&b), ["e"]);
}

#[test]
fn validation_errors_are_displayable_without_exposing_identity_payloads() {
    assert_eq!(
        SelectionError::DuplicateIdentity { index: 2 }.to_string(),
        "selection order repeats an identity at index 2"
    );
    assert_eq!(
        SelectionError::TooManyItems {
            count: 3,
            maximum: 2
        }
        .to_string(),
        "selection order has 3 items; maximum is 2"
    );
}
