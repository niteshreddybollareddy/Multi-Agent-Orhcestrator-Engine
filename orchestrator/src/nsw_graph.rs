use crate::tensor::cosine_similarity;
use std::collections::{BinaryHeap, HashSet};
use core::cmp::Ordering;

#[derive(Clone)]
pub struct NswGraph {
    /// Each node contains its vector representation
    pub nodes: Vec<Vec<f32>>,
    /// Adjacency list storing edges to neighbor nodes (max M connections)
    pub edges: Vec<Vec<usize>>,
    /// Random entry point for searches
    pub entry_point: usize,
}

#[derive(PartialEq, Clone, Copy)]
struct OrderedFloat(f32);

impl Eq for OrderedFloat {}

impl PartialOrd for OrderedFloat {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        // Reverse order so BinaryHeap acts as a min-heap
        other.0.partial_cmp(&self.0)
    }
}

impl Ord for OrderedFloat {
    fn cmp(&self, other: &Self) -> Ordering {
        self.partial_cmp(other).unwrap_or(Ordering::Equal)
    }
}

impl NswGraph {
    pub fn new() -> Self {
        Self {
            nodes: Vec::new(),
            edges: Vec::new(),
            entry_point: 0,
        }
    }

    /// Calculates similarity distance (higher similarity = lower distance metric)
    fn distance(a: &[f32], b: &[f32]) -> f32 {
        // cosine_similarity is 1 for identical vectors, so 1 - similarity acts as a distance
        1.0 - cosine_similarity(a, b)
    }

    /// Greedy search in Navigable Small World
    /// Starting from the entry point, it moves to the neighbor closest to the query
    /// until it hits a local minimum.
    pub fn search(&self, query: &[f32], ef_search: usize) -> Vec<(usize, f32)> {
        if self.nodes.is_empty() {
            return vec![];
        }

        let mut candidates = BinaryHeap::new();
        let mut results = BinaryHeap::new();
        let mut visited = HashSet::new();

        let dist = Self::distance(&self.nodes[self.entry_point], query);
        
        candidates.push((OrderedFloat(dist), self.entry_point));
        results.push((OrderedFloat(dist), self.entry_point));
        visited.insert(self.entry_point);

        while let Some((candidate_dist, current_node)) = candidates.pop() {
            // Stop if the best candidate is further away than the worst result in our list
            if let Some((worst_result_dist, _)) = results.peek() {
                if candidate_dist.0 > worst_result_dist.0 && results.len() >= ef_search {
                    break;
                }
            }

            for &neighbor in &self.edges[current_node] {
                if !visited.insert(neighbor) {
                    continue; // Already visited
                }

                let neighbor_dist = Self::distance(&self.nodes[neighbor], query);

                // We want to keep track of the best results
                let mut should_add = false;
                if results.len() < ef_search {
                    should_add = true;
                } else if let Some((worst_result_dist, _)) = results.peek() {
                    if neighbor_dist < worst_result_dist.0 {
                        should_add = true;
                    }
                }

                if should_add {
                    candidates.push((OrderedFloat(neighbor_dist), neighbor));
                    results.push((OrderedFloat(neighbor_dist), neighbor));
                    
                    if results.len() > ef_search {
                        results.pop(); // Remove the furthest node
                    }
                }
            }
        }

        // Extract results and sort by closest
        let mut final_results: Vec<(usize, f32)> = results.into_iter()
            .map(|(dist, id)| (id, 1.0 - dist.0)) // convert distance back to similarity
            .collect();
        
        final_results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(Ordering::Equal));
        final_results
    }
}
