// Rust guideline compliant 2026-10-16

//! Imports three linked tasks, recovers its receipt, and schedules the selected set.

use pearls_app::{
    service::{BatchRequest, CreateTask, GraphService, Mutation, TaskRef, TaskSelection},
    AppError,
};
use pearls_core::DepType;

fn main() -> Result<(), AppError> {
    let root = std::env::args().nth(1).ok_or_else(|| {
        AppError::InvalidInput("Usage: graph_service <initialized-repository-root>".into())
    })?;
    let service = GraphService::open(root)?;
    let key = "example:plan:v1";
    let expected_revision = match service.receipt(key)? {
        Some(receipt) => receipt.before_revision,
        None => service.snapshot()?.revision,
    };
    let mut mutations = Vec::new();
    for (local_ref, title) in [
        ("design", "Design the feature"),
        ("build", "Implement the feature"),
        ("review", "Review the implementation"),
    ] {
        mutations.push(Mutation::Create {
            local_ref: local_ref.into(),
            task: CreateTask {
                title: title.into(),
                author: "example-client".into(),
                description: String::new(),
                priority: None,
                labels: vec!["example".into()],
                metadata: Default::default(),
                external_key: Some(format!("example:plan:{local_ref}")),
            },
        });
    }
    for (from, to) in [("build", "design"), ("review", "build")] {
        mutations.push(Mutation::Link {
            from: TaskRef::Local(from.into()),
            to: TaskRef::Local(to.into()),
            dep_type: DepType::Blocks,
        });
    }
    // A real client persists the complete request before sending it.
    let request = BatchRequest {
        operation_key: key.into(),
        expected_revision,
        mutations,
    };
    let receipt = service.apply(&request)?;
    let selection = TaskSelection {
        required_ids: receipt.created_ids.values().cloned().collect(),
    };
    let schedule = service.schedule(&selection)?;
    println!(
        "{} ready; {} running; {} blocked; {} deferred; {} closed",
        schedule.ready.len(),
        schedule.running.len(),
        schedule.blocked.len(),
        schedule.deferred.len(),
        schedule.closed.len(),
    );
    Ok(())
}
