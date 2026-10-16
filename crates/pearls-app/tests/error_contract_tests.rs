// Rust guideline compliant 2026-10-15

//! Structured error envelopes for graph and status validation failures.

use pearls_app::{AppError, ErrorCode, ErrorEnvelope};
use pearls_core::{
    DepType, GraphError, Status, TransitionBlocker, TransitionDenial, TransitionDenialReason,
};

#[test]
fn missing_endpoint_details_are_available_without_message_parsing() {
    let error = AppError::from(pearls_core::Error::from(GraphError::MissingEndpoint {
        from_id: "prl-000001".into(),
        to_id: "prl-000002".into(),
        dep_type: DepType::Blocks,
    }));
    let envelope = ErrorEnvelope::from_error(&error);
    assert_eq!(envelope.code, ErrorCode::InvalidGraph);
    let json = serde_json::to_value(envelope).unwrap();
    assert_eq!(json["code"], "invalid_graph");
    assert_eq!(json["details"]["kind"], "missing_endpoint");
    assert_eq!(json["details"]["from_id"], "prl-000001");
    assert_eq!(json["details"]["to_id"], "prl-000002");
    assert_eq!(json["details"]["dep_type"], "blocks");
}

#[test]
fn cycle_path_uses_invalid_graph_code() {
    let cycle = vec![
        "prl-000001".to_string(),
        "prl-000002".to_string(),
        "prl-000001".to_string(),
    ];
    let envelope = ErrorEnvelope::from_error(&AppError::from(pearls_core::Error::CycleDetected(
        cycle.clone(),
    )));
    assert_eq!(envelope.code, ErrorCode::InvalidGraph);
    assert_eq!(
        envelope.details.unwrap()["cycle"],
        serde_json::to_value(cycle).unwrap()
    );
}

#[test]
fn transition_denial_exposes_reason_and_blocker_status() {
    let error = AppError::from(pearls_core::Error::from(TransitionDenial {
        task_id: "prl-000001".into(),
        from_status: Status::Open,
        to_status: Status::InProgress,
        reason: TransitionDenialReason::Blocked,
        blockers: vec![TransitionBlocker {
            id: "prl-000002".into(),
            status: Status::Deferred,
        }],
    }));
    let envelope = ErrorEnvelope::from_error(&error);
    assert_eq!(envelope.code, ErrorCode::InvalidTransition);
    let details = envelope.details.unwrap();
    assert_eq!(details["reason"], "blocked");
    assert_eq!(details["task_id"], "prl-000001");
    assert_eq!(details["to_status"], "in_progress");
    assert_eq!(details["blockers"][0]["id"], "prl-000002");
    assert_eq!(details["blockers"][0]["status"], "deferred");
}
