use swrm::tab_labels::unique_label;

#[test]
fn returns_base_when_no_existing() {
    let empty: &[&str] = &[];
    assert_eq!(unique_label(empty, "claude"), "claude");
    assert_eq!(unique_label(&["other"], "claude"), "claude");
}

#[test]
fn appends_two_when_base_is_taken() {
    assert_eq!(unique_label(&["claude"], "claude"), "claude 2");
}

#[test]
fn fills_gaps_after_delete() {
    assert_eq!(unique_label(&["claude", "claude 3"], "claude"), "claude 2");
}

#[test]
fn continues_past_three() {
    assert_eq!(
        unique_label(&["claude", "claude 2", "claude 3"], "claude"),
        "claude 4"
    );
}

#[test]
fn different_kinds_are_independent() {
    assert_eq!(
        unique_label(&["terminal", "claude"], "terminal"),
        "terminal 2"
    );
    assert_eq!(unique_label(&["terminal", "claude"], "claude"), "claude 2");
}

/// Locks in that we treat `existing` labels as opaque strings — a label with
/// a non-numeric suffix doesn't block the numeric one.
#[test]
fn non_numeric_suffix_is_not_a_match() {
    assert_eq!(unique_label(&["claude", "claude 2x"], "claude"), "claude 2");
}

/// Pin the slice-of-owned-strings calling convention used by the only real
/// caller (`MainTabsPanel`), where tab labels live as `String` on the tab
/// struct and the collected slice may be `&[String]` or `&[&String]`.
#[test]
fn accepts_slice_of_strings() {
    let labels: Vec<String> = vec!["claude".into(), "claude 2".into()];
    assert_eq!(unique_label(&labels, "claude"), "claude 3");
}
