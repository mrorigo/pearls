// Rust guideline compliant 2026-02-06

//! Graph module for dependency management and DAG operations.
//!
//! This module provides functionality for managing dependencies between Pearls,
//! including cycle detection, topological sorting, and ready queue computation.

use crate::{DepType, Error, Pearl, Result, Status};
use petgraph::algo::{is_cyclic_directed, toposort};
use petgraph::graph::{DiGraph, NodeIndex};
use rayon::prelude::*;
use std::collections::HashMap;

/// Issue graph for dependency management.
///
/// Maintains a directed acyclic graph (DAG) of Pearl dependencies with
/// efficient cycle detection and topological sorting capabilities.
pub struct IssueGraph {
    /// Petgraph directed graph where nodes are Pearl IDs and edges are dependencies.
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
    /// Returns an error if the dependency graph contains cycles.
    pub fn from_pearls(pearls: Vec<Pearl>) -> Result<Self> {
        let mut graph = DiGraph::new();
        let mut id_to_node = HashMap::new();
        let pearls_map: HashMap<String, Pearl> =
            pearls.iter().map(|p| (p.id.clone(), p.clone())).collect();

        // Add all Pearls as nodes
        for pearl in &pearls {
            let node_idx = graph.add_node(pearl.id.clone());
            id_to_node.insert(pearl.id.clone(), node_idx);
        }

        // Add all dependencies as edges
        for pearl in &pearls {
            if let Some(&from_idx) = id_to_node.get(&pearl.id) {
                for dep in &pearl.deps {
                    if let Some(&to_idx) = id_to_node.get(&dep.target_id) {
                        graph.add_edge(from_idx, to_idx, dep.dep_type);
                    }
                }
            }
        }

        // Check for cycles
        if is_cyclic_directed(&graph) {
            if let Some(cycle) = Self::find_cycle_internal(&graph, &id_to_node) {
                return Err(Error::CycleDetected(cycle));
            }
        }

        Ok(Self {
            graph,
            id_to_node,
            pearls: pearls_map,
        })
    }

    /// Adds a dependency between two Pearls with cycle detection.
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

        // Add the edge temporarily
        self.graph.add_edge(from_idx, to_idx, dep_type);

        // Check for cycles
        if is_cyclic_directed(&self.graph) {
            // Remove the edge if it creates a cycle
            if let Some(edge_idx) = self.graph.find_edge(from_idx, to_idx) {
                self.graph.remove_edge(edge_idx);
            }
            if let Some(cycle) = Self::find_cycle_internal(&self.graph, &self.id_to_node) {
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

    /// Removes a dependency between two Pearls.
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
        if let Some(edge_idx) = self.graph.find_edge(from_idx, to_idx) {
            self.graph.remove_edge(edge_idx);
        }

        // Remove from Pearl's dependencies
        if let Some(pearl) = self.pearls.get_mut(from_id) {
            pearl.deps.retain(|dep| dep.target_id != to_id);
        }

        Ok(())
    }

    /// Checks if the graph contains cycles.
    ///
    /// # Returns
    ///
    /// True if the graph is cyclic, false if it's a DAG.
    pub fn has_cycle(&self) -> bool {
        is_cyclic_directed(&self.graph)
    }

    /// Finds a cycle in the graph if one exists.
    ///
    /// # Returns
    ///
    /// Some(cycle_path) if a cycle exists, None otherwise.
    /// The cycle_path is a vector of Pearl IDs forming the cycle.
    pub fn find_cycle(&self) -> Option<Vec<String>> {
        Self::find_cycle_internal(&self.graph, &self.id_to_node)
    }

    /// Internal helper to find a cycle using DFS.
    fn find_cycle_internal(
        graph: &DiGraph<String, DepType>,
        _id_to_node: &HashMap<String, NodeIndex>,
    ) -> Option<Vec<String>> {
        use petgraph::visit::Dfs;

        // Try to find a cycle by attempting topological sort
        // If it fails, we know there's a cycle
        if let Err(_) = toposort(graph, None) {
            // Simple cycle detection: do DFS and look for back edges
            for start_node in graph.node_indices() {
                let mut dfs = Dfs::new(graph, start_node);
                let mut visited = std::collections::HashSet::new();
                let mut rec_stack = std::collections::HashSet::new();

                while let Some(node) = dfs.next(graph) {
                    if rec_stack.contains(&node) {
                        // Found a back edge, construct the cycle
                        let mut cycle = vec![graph[node].clone()];

                        // Walk back to find the cycle
                        for neighbor in graph.neighbors(node) {
                            if neighbor == node {
                                cycle.push(graph[node].clone());
                                return Some(cycle);
                            }
                        }
                    }

                    visited.insert(node);
                    rec_stack.insert(node);
                }

                for node in visited {
                    rec_stack.remove(&node);
                }
            }

            // Fallback: return a simple cycle representation
            return Some(vec!["cycle_detected".to_string()]);
        }

        None
    }

    /// Performs topological sort on the graph.
    ///
    /// # Returns
    ///
    /// Ok(sorted_ids) with Pearl IDs in topological order, or Err if graph is cyclic.
    ///
    /// # Errors
    ///
    /// Returns an error if the graph contains cycles.
    pub fn topological_sort(&self) -> Result<Vec<String>> {
        match toposort(&self.graph, None) {
            Ok(sorted_nodes) => {
                let sorted_ids = sorted_nodes
                    .iter()
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
