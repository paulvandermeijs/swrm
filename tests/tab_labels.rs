use swrm::tab_labels::unique_label;

#[test]
fn returns_base_when_no_existing() {
    assert_eq!(unique_label(&[], "claude"), "claude");
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
