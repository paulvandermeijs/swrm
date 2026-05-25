use swrm::agent_status::AgentStatus;

#[test]
fn agent_status_priority_orders_attention_first() {
    // Higher priority = more urgent. Notify is the most attention-worthy.
    assert!(AgentStatus::Notify.priority() > AgentStatus::Done.priority());
    assert!(AgentStatus::Done.priority() > AgentStatus::Working.priority());
    assert!(AgentStatus::Working.priority() > AgentStatus::Idle.priority());
}

#[test]
fn agent_status_from_str_round_trips_known() {
    for &(s, expected) in &[
        ("notify", AgentStatus::Notify),
        ("done", AgentStatus::Done),
        ("working", AgentStatus::Working),
        ("idle", AgentStatus::Idle),
    ] {
        assert_eq!(AgentStatus::from_wire(s), Some(expected));
    }
}

#[test]
fn agent_status_from_str_unknown_is_none() {
    assert_eq!(AgentStatus::from_wire(""), None);
    assert_eq!(AgentStatus::from_wire("bogus"), None);
    assert_eq!(AgentStatus::from_wire("NOTIFY"), None); // case-sensitive
}
