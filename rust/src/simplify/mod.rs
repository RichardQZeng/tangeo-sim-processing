mod dp;
mod engine;

use crate::geometry::FeatureRecord;

pub use dp::{find_farthest_point, init_process_line_stack, point_to_segment_dist};
pub use engine::SimplifyEngine;

#[derive(Debug, Clone, Copy)]
pub struct SimplifyParams {
    pub tolerance: f64,
    pub validate_structure: bool,
}

#[derive(Debug, Clone)]
pub struct SimplifyStats {
    pub in_nbr_features: usize,
    pub out_nbr_features: usize,
    pub nbr_vertice_deleted: usize,
    pub nbr_pass: usize,
    pub is_structure_valid: Option<bool>,
}

#[derive(Debug, Clone)]
pub struct SimplifyOutput {
    pub features: Vec<FeatureRecord>,
    pub stats: SimplifyStats,
}

#[derive(Debug, Clone, Copy)]
pub struct SimplifyProgress {
    pub current_feature: usize,
    pub total_features: usize,
    pub current_pass: usize,
}
