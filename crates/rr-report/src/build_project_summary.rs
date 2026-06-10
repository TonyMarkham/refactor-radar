use rr_core::{ProjectReportSummary, SemanticModel};

use std::collections::BTreeMap;

pub fn build_project_summary(model: &SemanticModel) -> ProjectReportSummary {
    let mut reference_file_counts = BTreeMap::new();
    for reference in &model.references {
        let count = reference_file_counts
            .entry(reference.document_path.clone())
            .or_insert(0_usize);
        *count += 1;
    }

    let mut hotspot_files: Vec<_> = reference_file_counts.into_iter().collect();
    hotspot_files.sort_by(|left, right| right.1.cmp(&left.1).then_with(|| left.0.cmp(&right.0)));

    ProjectReportSummary {
        project_id: model.project.project_id.clone(),
        document_count: model.files.len(),
        element_count: model.elements.len(),
        reference_count: model.references.len(),
        hotspot_files: hotspot_files
            .into_iter()
            .map(|(path, count)| format!("{path} ({count})"))
            .collect(),
    }
}
