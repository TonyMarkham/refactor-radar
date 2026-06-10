use rr_core::SemanticModel;

use std::{collections::HashMap, path::PathBuf};

#[derive(Debug, Default)]
pub struct ProjectCache {
    pub models: HashMap<String, SemanticModel>,
    pub scip_paths: HashMap<String, PathBuf>,
    pub temporary_scip_paths: HashMap<String, tempfile::TempPath>,
    pub producer_metadata: HashMap<String, String>,
    pub generation_diagnostics: HashMap<String, String>,
}

impl ProjectCache {
    pub fn insert(
        &mut self,
        model: SemanticModel,
        scip_path: PathBuf,
        diagnostics: Option<String>,
        temporary_scip_path: Option<tempfile::TempPath>,
    ) {
        let project_id = model.project.project_id.clone();
        if let Some(producer_name) = &model.project.producer_name {
            self.producer_metadata
                .insert(project_id.clone(), producer_name.clone());
        }
        if let Some(diagnostics) = diagnostics {
            self.generation_diagnostics
                .insert(project_id.clone(), diagnostics);
        }
        match temporary_scip_path {
            Some(temporary_scip_path) => {
                self.temporary_scip_paths
                    .insert(project_id.clone(), temporary_scip_path);
            }
            None => {
                self.temporary_scip_paths.remove(&project_id);
            }
        }
        self.scip_paths.insert(project_id.clone(), scip_path);
        self.models.insert(project_id, model);
    }
}
