// Rust guideline compliant 2026-10-15

//! In-memory validation and mutation, invoked inside the repository transaction.

use super::{CreateTask, Mutation, TaskRef, EXTERNAL_DEFINITION, EXTERNAL_KEY};
use crate::{AppError, Result};
use pearls_core::{repository::RepositoryData, Config, Dependency, IssueGraph, Pearl};
use serde_json::Value;
use std::collections::{BTreeMap, HashMap};

fn all(data: &RepositoryData) -> impl Iterator<Item = &Pearl> {
    data.active.iter().chain(&data.archived)
}

pub(super) fn validate(data: &RepositoryData) -> Result<IssueGraph> {
    let graph = IssueGraph::from_pearls_strict(all(data).cloned().collect())?;
    let mut keys = HashMap::new();
    for pearl in all(data) {
        if let Some(value) = pearl.metadata.get(EXTERNAL_KEY) {
            let key = value
                .as_str()
                .ok_or_else(|| AppError::InvalidExternalIdentity {
                    task_id: pearl.id.clone(),
                    reason: "External key must be a string".into(),
                })?;
            if let Some(previous) = keys.insert(key, &pearl.id) {
                return Err(AppError::ExternalKeyConflict {
                    key: key.into(),
                    task_ids: vec![previous.clone(), pearl.id.clone()],
                });
            }
        }
        if let Some(definition) = pearl.metadata.get(EXTERNAL_DEFINITION) {
            let task: CreateTask = serde_json::from_value(definition.clone()).map_err(|_| {
                AppError::InvalidExternalIdentity {
                    task_id: pearl.id.clone(),
                    reason: "External definition is malformed".into(),
                }
            })?;
            if task.external_key.is_none()
                || task.external_key.as_deref().map(Value::from).as_ref()
                    != pearl.metadata.get(EXTERNAL_KEY)
            {
                return Err(AppError::InvalidExternalIdentity {
                    task_id: pearl.id.clone(),
                    reason: "External definition and key disagree".into(),
                });
            }
        }
    }
    Ok(graph)
}

fn resolve(
    reference: &TaskRef,
    locals: &BTreeMap<String, String>,
    data: &RepositoryData,
) -> Result<String> {
    let id = match reference {
        TaskRef::Id(id) => id,
        TaskRef::Local(key) => locals
            .get(key)
            .ok_or_else(|| AppError::InvalidInput(format!("Unknown local reference: {key}")))?,
    };
    if !all(data).any(|pearl| pearl.id == *id) {
        return Err(AppError::TaskNotFound { id: id.clone() });
    }
    Ok(id.clone())
}

fn active<'a>(data: &'a mut RepositoryData, id: &str) -> Result<&'a mut Pearl> {
    data.active
        .iter_mut()
        .find(|pearl| pearl.id == id)
        .ok_or_else(|| AppError::InvalidInput(format!("Archived task cannot be edited: {id}")))
}

fn check_content(description: &str, priority: u8, config: &Config) -> Result<()> {
    if description.len() > 64 * 1024 {
        return Err(AppError::InvalidInput(
            "Description exceeds 64KB limit".into(),
        ));
    }
    if priority > config.max_priority {
        return Err(AppError::InvalidInput(format!(
            "Priority must be 0-{}, got {}",
            config.max_priority, priority
        )));
    }
    Ok(())
}

fn reserved(key: &str) -> Result<()> {
    if key.is_empty() || key == EXTERNAL_KEY || key == EXTERNAL_DEFINITION {
        return Err(AppError::InvalidInput(format!(
            "Invalid or reserved metadata key: {key}"
        )));
    }
    Ok(())
}

fn create(data: &mut RepositoryData, task: &CreateTask, config: &Config) -> Result<String> {
    let definition = serde_json::to_value(task).map_err(pearls_core::Error::from)?;
    if let Some(key) = &task.external_key {
        if let Some(existing) =
            all(data).find(|p| p.metadata.get(EXTERNAL_KEY) == Some(&Value::String(key.clone())))
        {
            if existing.metadata.get(EXTERNAL_DEFINITION) == Some(&definition) {
                return Ok(existing.id.clone());
            }
            return Err(AppError::ExternalKeyConflict {
                key: key.clone(),
                task_ids: vec![existing.id.clone()],
            });
        }
    }
    for key in task.metadata.keys() {
        reserved(key)?;
    }
    let priority = task.priority.unwrap_or(config.default_priority);
    check_content(&task.description, priority, config)?;
    let now = crate::unix_timestamp()?;
    let mut pearl = Pearl::new(task.title.clone(), task.author.clone());
    pearl.created_at = now;
    pearl.updated_at = now;
    pearl.id = (0..65_536)
        .map(|nonce| pearls_core::identity::generate_id(&task.title, &task.author, now, nonce))
        .find(|id| !all(data).any(|existing| existing.id == *id))
        .ok_or_else(|| AppError::InvalidInput("Task ID candidate space exhausted".into()))?;
    pearl.description = task.description.clone();
    pearl.priority = priority;
    pearl.labels = task.labels.clone();
    pearl.metadata = task.metadata.clone().into_iter().collect();
    if let Some(key) = &task.external_key {
        pearl
            .metadata
            .insert(EXTERNAL_KEY.into(), Value::String(key.clone()));
        pearl
            .metadata
            .insert(EXTERNAL_DEFINITION.into(), definition);
    }
    pearl.validate()?;
    let id = pearl.id.clone();
    data.active.push(pearl);
    Ok(id)
}

pub(super) fn apply(
    data: &mut RepositoryData,
    mutations: &[Mutation],
    config: &Config,
) -> Result<BTreeMap<String, String>> {
    validate(data)?;
    let mut locals = BTreeMap::new();
    for mutation in mutations {
        match mutation {
            Mutation::Import { tasks } => {
                let before = data.clone();
                for task in tasks {
                    check_content(&task.description, task.priority, config)?;
                    if data.archived.iter().any(|archived| archived.id == task.id) {
                        return Err(AppError::InvalidInput(format!(
                            "Cannot import over archived task {}",
                            task.id
                        )));
                    }
                    if let Some(position) = data
                        .active
                        .iter()
                        .position(|existing| existing.id == task.id)
                    {
                        for key in [EXTERNAL_KEY, EXTERNAL_DEFINITION] {
                            if data.active[position].metadata.get(key) != task.metadata.get(key) {
                                return Err(AppError::InvalidInput(format!(
                                    "Import cannot replace reserved metadata: {key}"
                                )));
                            }
                        }
                        data.active[position] = task.clone();
                    } else {
                        data.active.push(task.clone());
                    }
                }
                let graph = validate(data)?;
                for previous in &before.active {
                    if let Some(current) = data.active.iter().find(|task| task.id == previous.id) {
                        if previous.status != current.status {
                            pearls_core::validate_transition(previous, current.status, &graph)?;
                        }
                    }
                }
            }
            Mutation::Create { local_ref, task } => {
                if local_ref.is_empty() || locals.contains_key(local_ref) {
                    return Err(AppError::InvalidInput(format!(
                        "Empty or duplicate local reference: {local_ref}"
                    )));
                }
                let id = create(data, task, config)?;
                locals.insert(local_ref.clone(), id);
            }
            Mutation::Update { task, patch } => {
                let id = resolve(task, &locals, data)?;
                let pearl = active(data, &id)?;
                if let Some(title) = &patch.title {
                    pearl.title.clone_from(title);
                }
                if let Some(description) = &patch.description {
                    pearl.description.clone_from(description);
                }
                if let Some(priority) = patch.priority {
                    pearl.priority = priority;
                }
                if let Some(labels) = &patch.labels {
                    pearl.labels.clone_from(labels);
                }
                check_content(
                    patch.description.as_deref().unwrap_or(""),
                    patch.priority.unwrap_or(0),
                    config,
                )?;
                pearl.updated_at = crate::unix_timestamp()?;
            }
            Mutation::SetStatus { task, status } => {
                let id = resolve(task, &locals, data)?;
                let graph = validate(data)?;
                let pearl = active(data, &id)?;
                pearls_core::validate_transition(pearl, *status, &graph)?;
                pearl.status = *status;
                pearl.updated_at = crate::unix_timestamp()?;
            }
            Mutation::Link { from, to, dep_type } => {
                let from = resolve(from, &locals, data)?;
                let to = resolve(to, &locals, data)?;
                let pearl = active(data, &from)?;
                let dep = Dependency {
                    target_id: to,
                    dep_type: *dep_type,
                };
                if !pearl.deps.contains(&dep) {
                    pearl.deps.push(dep);
                }
                pearl.updated_at = crate::unix_timestamp()?;
            }
            Mutation::Unlink { from, to } => {
                let from = resolve(from, &locals, data)?;
                let to = resolve(to, &locals, data)?;
                let pearl = active(data, &from)?;
                pearl.deps.retain(|dep| dep.target_id != to);
                pearl.updated_at = crate::unix_timestamp()?;
            }
            Mutation::SetMetadata { task, key, value } => {
                reserved(key)?;
                let id = resolve(task, &locals, data)?;
                let pearl = active(data, &id)?;
                pearl.metadata.insert(key.clone(), value.clone());
                pearl.updated_at = crate::unix_timestamp()?;
            }
            Mutation::AddComment { task, author, body } => {
                let id = resolve(task, &locals, data)?;
                active(data, &id)?.add_comment(author.clone(), body.clone())?;
            }
            Mutation::DeleteComment { task, comment_id } => {
                let id = resolve(task, &locals, data)?;
                if !active(data, &id)?.delete_comment(comment_id) {
                    return Err(AppError::TaskNotFound {
                        id: comment_id.clone(),
                    });
                }
            }
        }
        validate(data)?;
    }
    Ok(locals)
}
