use crate::{CoreError, CoreResult, SpanId};

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SourceSpan {
    pub id: SpanId,
    pub project_id: String,
    pub document_path: String,
    pub start_line: u32,
    pub start_column: u32,
    pub end_line: u32,
    pub end_column: u32,
}

impl SourceSpan {
    pub fn from_scip_range(
        project_id: impl Into<String>,
        document_path: impl Into<String>,
        range: &[i32],
    ) -> CoreResult<Self> {
        let project_id = project_id.into();
        let document_path = document_path.into();
        let positions = match range {
            [line, start, end] => (*line, *start, *line, *end),
            [start_line, start_column, end_line, end_column] => {
                (*start_line, *start_column, *end_line, *end_column)
            }
            _ => return Err(CoreError::invalid_scip_range(range.to_vec())),
        };
        validate_scip_positions(positions, range)?;
        let start_line = one_based_position(positions.0, range)?;
        let start_column = one_based_position(positions.1, range)?;
        Ok(Self {
            id: SpanId::new(&project_id, &document_path, start_line, start_column),
            project_id,
            document_path,
            start_line,
            start_column,
            end_line: one_based_position(positions.2, range)?,
            end_column: one_based_position(positions.3, range)?,
        })
    }
}

fn validate_scip_positions(positions: (i32, i32, i32, i32), range: &[i32]) -> CoreResult<()> {
    let (start_line, start_column, end_line, end_column) = positions;
    if start_line < 0
        || start_column < 0
        || end_column < 0
        || end_line < start_line
        || (end_line == start_line && end_column < start_column)
    {
        return Err(CoreError::invalid_scip_range(range.to_vec()));
    }
    Ok(())
}

fn one_based_position(position: i32, range: &[i32]) -> CoreResult<u32> {
    let position = position
        .checked_add(1)
        .ok_or_else(|| CoreError::invalid_scip_range(range.to_vec()))?;
    u32::try_from(position).map_err(|_| CoreError::invalid_scip_range(range.to_vec()))
}
