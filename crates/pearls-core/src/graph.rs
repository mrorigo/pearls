// Rust guideline compliant 2026-02-06

//! Graph module for dependency management and DAG operations.
//!
//! This module provides functionality for managing dependencies between Pearls,
//! including cycle detection, topological sorting, and ready queue computation.

use crate::{DepType, Error, Pearl, Result, Status};
use petgraph::algo::{is_cyclic_directed, toposort};
use petgraph::graph::{DiGraph, NodeIndex};
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Integrity failure when constructing a complete task graph.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, thiserror::Error)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum GraphError {
    /// Multiple tasks have the same identity.
    #[error("Duplicate Pearl ID: {id}")]
    DuplicateId {
        /// Identity shared by multiple tasks.
        id: String,
    },
    /// A dependency references a task absent from the complete snapshot.
    #[error("Missing dependency endpoint: {from_id} -> {to_id} ({dep_type:?})")]
    MissingEndpoint {
        /// Task containing the dependency.
        from_id: String,
        /// Missing dependency target.
        to_id: String,
        /// Relationship type whose endpoint is missing.
        dep_type: DepType,
    },
}

/// Issue graph for dependency management.
///
/// Maintains a directed acyclic graph of blocking dependencies. Non-blocking
/// relationships remain available through the stored Pearls and may contain
/// cycles; they do not constrain scheduling or status transitions.
pub struct IssueGraph {
    /// Directed graph containing only `Blocks` edges between Pearl IDs.
    graph: DiGraph<String, DepType>,
    /// Map of Pearl IDs to their NodeIndex in the graph.
    id_to_node: HashMap<String, NodeIndex>,
    /// Map of Pearl IDs to Pearl data.
    pearls: HashMap<String, Pearl>,
}

impl IssueGraph {
    /// Creates a new IssueGraph from a collection of Pearls.
    ///
    /// # Arguments
    ///
    /// * `pearls` - Vector of Pearls to build the graph from
    ///
    /// # Returns
    ///
    /// A new IssueGraph instance.
    ///
    /// # Errors
    ///
    /// Returns an error if blocking dependencies form a cycle.
    ///
    /// This legacy constructor tolerates missing endpoints, for callers reading
    /// only active tasks. New repository services should load referenced archives
    /// and use [`Self::from_pearls_strict`] instead.
    pub fn from_pearls(pearls: Vec<Pearl>) -> Result<Self> {
        Self::build(pearls, false)
    }

    /// Builds and validates a complete task graph, including archived endpoints.
    ///
    /// # Arguments
    ///
    /// * `pearls` - Complete task set, including referenced archived tasks.
    ///
    /// # Returns
    ///
    /// A graph whose blocking dependencies are acyclic and whose endpoints exist.
    ///
    /// # Errors
    ///
    /// Returns `InvalidGraph` for duplicate IDs or missing dependency endpoints,
    /// `CycleDetected` for a blocking cycle, or a task validation error.
    pub fn from_pearls_strict(pearls: Vec<Pearl>) -> Result<Self> {
        Self::build(pearls, true)
    }

    fn build(pearls: Vec<Pearl>, strict: bool) -> Result<Self> {
        let mut graph = DiGraph::new();
        let mut id_to_node = HashMap::new();
        let mut pearls_map = HashMap::new();
        for pearl in pearls {
            if strict {
                pearl.validate()?;
                if pearls_map.contains_key(&pearl.id) {
                    return Err(GraphError::DuplicateId { id: pearl.id }.into());
                }
            }
            let node = graph.add_node(pearl.id.clone());
            id_to_node.insert(pearl.id.clone(), node);
            pearls_map.insert(pearl.id.clone(), pearl);
        }

        // Preserve input node order for predictable traversal; HashMap
        // iteration must not choose which cycle is reported.
        for from in graph.node_indices().collect::<Vec<_>>() {
            let pearl = &pearls_map[&graph[from]];
            for dep in &pearl.deps {
                if let Some(&to) = id_to_node.get(&dep.target_id) {
                    if dep.dep_type == DepType::Blocks {
                        graph.add_edge(from, to, dep.dep_type);
                    }
                } else if strict {
                    return Err(GraphError::MissingEndpoint {
                        from_id: pearl.id.clone(),
                        to_id: dep.target_id.clone(),
                        dep_type: dep.dep_type,
                    }
                    .into());
                }
            }
        }
        if let Some(cycle) = Self::find_cycle_internal(&graph) {
            return Err(Error::CycleDetected(cycle));
        }
        Ok(Self {
            graph,
            id_to_node,
            pearls: pearls_map,
        })
    }

    /// Returns a task from this graph by its full ID.
    ///
    /// Archived tasks are available when included in the input snapshot.
    ///
    /// # Arguments
    ///
    /// * `id` - Full identity of the requested task.
    ///
    /// # Returns
    ///
    /// A task reference, or `None` when the task is absent.
    #[must_use]
    pub fn pearl(&self, id: &str) -> Option<&Pearl> {
        self.pearls.get(id)
    }

    /// Adds a dependency, rejecting cycles composed of blocking edges.
    ///
    /// # Arguments
    ///
    /// * `from_id` - The source Pearl ID
    /// * `to_id` - The target Pearl ID
    /// * `dep_type` - The type of dependency
    ///
    /// # Returns
    ///
    /// Ok if the dependency was added, Err if it would create a cycle or IDs are invalid.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Either Pearl ID does not exist
    /// - Adding the dependency would create a cycle
    pub fn add_dependency(&mut self, from_id: &str, to_id: &str, dep_type: DepType) -> Result<()> {
        let from_idx = self
            .id_to_node
            .get(from_id)
            .copied()
            .ok_or_else(|| Error::NotFound(format!("Pearl {} not found", from_id)))?;

        let to_idx = self
            .id_to_node
            .get(to_id)
            .copied()
            .ok_or_else(|| Error::NotFound(format!("Pearl {} not found", to_id)))?;

        if dep_type == DepType::Blocks {
            let edge = self.graph.add_edge(from_idx, to_idx, dep_type);
            if let Some(cycle) = Self::find_cycle_internal(&self.graph) {
                self.graph.remove_edge(edge);
                return Err(Error::CycleDetected(cycle));
            }
        }

        // Update the Pearl's dependencies
        if let Some(pearl) = self.pearls.get_mut(from_id) {
            pearl.deps.push(crate::Dependency {
                target_id: to_id.to_string(),
                dep_type,
            });
        }

        Ok(())
    }

    /// Removes all dependency types between two Pearls.
    ///
    /// # Arguments
    ///
    /// * `from_id` - The source Pearl ID
    /// * `to_id` - The target Pearl ID
    ///
    /// # Returns
    ///
    /// Ok if the dependency was removed, Err if IDs are invalid.
    ///
    /// # Errors
    ///
    /// Returns an error if either Pearl ID does not exist.
    pub fn remove_dependency(&mut self, from_id: &str, to_id: &str) -> Result<()> {
        let from_idx = self
            .id_to_node
            .get(from_id)
            .copied()
            .ok_or_else(|| Error::NotFound(format!("Pearl {} not found", from_id)))?;

        let to_idx = self
            .id_to_node
            .get(to_id)
            .copied()
            .ok_or_else(|| Error::NotFound(format!("Pearl {} not found", to_id)))?;

        // Remove the edge from the graph
        while let Some(edge_idx) = self.graph.find_edge(from_idx, to_idx) {
            self.graph.remove_edge(edge_idx);
        }

        // Remove from Pearl's dependencies
        if let Some(pearl) = self.pearls.get_mut(from_id) {
            pearl.deps.retain(|dep| dep.target_id != to_id);
        }

        Ok(())
    }

    /// Checks if blocking dependencies contain a cycle.
    ///
    /// # Returns
    ///
    /// True if the graph is cyclic, false if it's a DAG.
    pub fn has_cycle(&self) -> bool {
        is_cyclic_directed(&self.graph)
    }

    /// Finds a cycle of blocking dependencies if one exists.
    ///
    /// # Returns
    ///
    /// Some(cycle_path) if a cycle exists, None otherwise.
    /// The cycle_path is a vector of Pearl IDs forming the cycle.
    pub fn find_cycle(&self) -> Option<Vec<String>> {
        Self::find_cycle_internal(&self.graph)
    }

    // Iterative DFS keeps cycle detection safe for deeply nested imports.
    fn find_cycle_internal(graph: &DiGraph<String, DepType>) -> Option<Vec<String>> {
        let mut visited = vec![false; graph.node_count()];
        let mut active_positions = vec![None; graph.node_count()];
        for start in graph.node_indices() {
            if visited[start.index()] {
                continue;
            }
            visited[start.index()] = true;
            active_positions[start.index()] = Some(0);
            let mut stack = vec![(start, graph.neighbors(start))];
            while let Some((node, neighbors)) = stack.last_mut() {
                let current = *node;
                if let Some(next) = neighbors.next() {
                    if let Some(position) = active_positions[next.index()] {
                        let mut cycle: Vec<_> = stack[position..]
                            .iter()
                            .map(|(node, _)| graph[*node].clone())
                            .collect();
                        cycle.push(graph[next].clone());
                        return Some(cycle);
                    }
                    if !visited[next.index()] {
                        visited[next.index()] = true;
                        active_positions[next.index()] = Some(stack.len());
                        stack.push((next, graph.neighbors(next)));
                    }
                } else {
                    active_positions[current.index()] = None;
                    stack.pop();
                }
            }
        }
        None
    }

    /// Orders tasks so each blocking dependency precedes its dependents.
    ///
    /// # Returns
    ///
    /// Task IDs with blocking prerequisites first. Non-blocking relationships
    /// do not affect the order.
    ///
    /// # Errors
    ///
    /// Returns an error if the graph contains cycles.
    pub fn topological_sort(&self) -> Result<Vec<String>> {
        match toposort(&self.graph, None) {
            Ok(sorted_nodes) => {
                let sorted_ids = sorted_nodes
                    .iter()
                    .rev()
                    .map(|node_idx| self.graph[*node_idx].clone())
                    .collect();
                Ok(sorted_ids)
            }
            Err(_) => {
                if let Some(cycle) = self.find_cycle() {
                    Err(Error::CycleDetected(cycle))
                } else {
                    Err(Error::CycleDetected(vec!["unknown_cycle".to_string()]))
                }
            }
        }
    }

    /// Checks if a Pearl is blocked by open blocking dependencies.
    ///
    /// # Arguments
    ///
    /// * `id` - The Pearl ID to check
    ///
    /// # Returns
    ///
    /// True if the Pearl has open blocking dependencies, false otherwise.
    pub fn is_blocked(&self, id: &str) -> bool {
        self.dependencies_by_type(id, DepType::Blocks)
            .into_iter()
            .any(|target| target.status != Status::Closed)
    }

    /// Returns the list of Pearls that are blocking the given Pearl.
    ///
    /// # Arguments
    ///
    /// * `id` - The Pearl ID to check
    ///
    /// # Returns
    ///
    /// Vector of references to blocking Pearls.
    pub fn blocking_deps(&self, id: &str) -> Vec<&Pearl> {
        self.dependencies_by_type(id, DepType::Blocks)
            .into_iter()
            .filter(|target| target.status != Status::Closed)
            .collect()
    }

    /// Returns dependencies for a given Pearl filtered by dependency type.
    ///
    /// # Arguments
    ///
    /// * `id` - The Pearl ID to query
    /// * `dep_type` - The dependency type to filter by
    ///
    /// # Returns
    ///
    /// Vector of references to dependencies matching the given type.
    pub fn dependencies_by_type(&self, id: &str, dep_type: DepType) -> Vec<&Pearl> {
        let mut deps = Vec::new();

        if let Some(pearl) = self.pearls.get(id) {
            for dep in &pearl.deps {
                if dep.dep_type == dep_type {
                    if let Some(target) = self.pearls.get(&dep.target_id) {
                        deps.push(target);
                    }
                }
            }
        }

        deps
    }

    /// Returns the ready queue: Pearls that are unblocked and ready for work.
    ///
    /// The ready queue includes all Pearls that:
    /// - Have status Open or InProgress (not Closed or Deferred)
    /// - Have zero open blocking dependencies
    ///
    /// Results are sorted by priority ascending (P0 first), then by updated_at descending.
    ///
    /// # Returns
    ///
    /// Vector of references to ready Pearls, sorted by priority and recency.
    pub fn ready_queue(&self) -> Vec<&Pearl> {
        const PARALLEL_THRESHOLD: usize = 1_000;

        let mut ready: Vec<&Pearl> = if self.pearls.len() >= PARALLEL_THRESHOLD {
            self.pearls
                .par_iter()
                .filter_map(|(_, pearl)| {
                    if pearl.status == Status::Closed || pearl.status == Status::Deferred {
                        return None;
                    }
                    if self.is_blocked(&pearl.id) {
                        return None;
                    }
                    Some(pearl)
                })
                .collect()
        } else {
            self.pearls
                .values()
                .filter(|pearl| pearl.status != Status::Closed && pearl.status != Status::Deferred)
                .filter(|pearl| !self.is_blocked(&pearl.id))
                .collect()
        };

        // Sort by priority ascending, then by updated_at descending
        ready.sort_by(|a, b| match a.priority.cmp(&b.priority) {
            std::cmp::Ordering::Equal => b.updated_at.cmp(&a.updated_at),
            other => other,
        });

        ready
    }
}
