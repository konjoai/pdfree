//! Structured extraction: tables from vector-drawn grids (Phase 6).
//!
//! Table cells come from `pdfree_core::boxes` — the same lattice-based
//! ruled-line cell reconstruction that powers the macOS app's box-on-load
//! scan — clustered into rows/columns by position. Cell contents come from
//! `pdfree_core::editor::text_runs`, matched to a cell by whether the run's
//! center point falls inside the cell's box. This is a specialized,
//! geometry-driven extractor, not an LLM call — matching the "specialized
//! extractors + optional LLM validation, not LLM alone" design principle
//! for this module. Contract analysis (LLM-driven, over free-form text) is
//! a separate, not-yet-implemented concern — see the open item in
//! CLAUDE.md's Phase 6 checklist.

use crate::Result;
use pdfree_core::{boxes, editor, Document};

/// PDF points within which two box y-centers are treated as being in the
/// same row — absorbs sub-point jitter in "aligned" ruled lines, the same
/// tolerance class `boxes::DetectedBox`'s own clustering uses internally.
const ROW_TOLERANCE: f32 = 3.0;

/// A page needs at least this many detected boxes before it's worth
/// clustering into rows at all — below this, `boxes_on_page`'s standalone-
/// rectangle inclusion (checkboxes, signature boxes) dominates and there's
/// no grid to find.
const MIN_CELLS_FOR_A_TABLE: usize = 4;

/// Extract every ruled-line table on every page of a document, as a list of
/// tables (each table a list of rows, each row a list of cell strings, top-
/// to-bottom then left-to-right). Pages with no reconstructable grid (fewer
/// than 2 rows, or no row with at least 2 columns) contribute nothing.
pub fn extract_tables(pdf_bytes: &[u8]) -> Result<Vec<Vec<Vec<String>>>> {
    let page_count = Document::from_bytes(pdf_bytes.to_vec(), None)?.page_count();
    let runs = editor::text_runs(pdf_bytes)?;

    let mut tables = Vec::new();
    for page in 0..page_count {
        let cells = boxes::boxes_on_page(pdf_bytes, page)?;
        if let Some(table) = table_from_cells(&cells, &runs) {
            tables.push(table);
        }
    }
    Ok(tables)
}

fn table_from_cells(
    cells: &[boxes::DetectedBox],
    runs: &[editor::TextRun],
) -> Option<Vec<Vec<String>>> {
    if cells.len() < MIN_CELLS_FOR_A_TABLE {
        return None;
    }

    // Top-to-bottom reading order: PDF y increases upward (bottom-left
    // origin), so descending y is top-to-bottom.
    let mut sorted: Vec<&boxes::DetectedBox> = cells.iter().collect();
    sorted.sort_by(|a, b| b.y.partial_cmp(&a.y).unwrap());

    let mut rows: Vec<Vec<&boxes::DetectedBox>> = Vec::new();
    for cell in sorted {
        match rows.last_mut() {
            Some(row) if (row[0].y - cell.y).abs() <= ROW_TOLERANCE => row.push(cell),
            _ => rows.push(vec![cell]),
        }
    }

    for row in &mut rows {
        row.sort_by(|a, b| a.x.partial_cmp(&b.x).unwrap());
    }

    let max_row_len = rows.iter().map(Vec::len).max().unwrap_or(0);
    if rows.len() < 2 || max_row_len < 2 {
        return None;
    }

    Some(
        rows.into_iter()
            .map(|row| row.into_iter().map(|cell| cell_text(cell, runs)).collect())
            .collect(),
    )
}

/// Every text run whose center point falls inside `cell`, joined in
/// document order. A cell with no overlapping run (blank on the source
/// page) yields an empty string, not an error.
fn cell_text(cell: &boxes::DetectedBox, runs: &[editor::TextRun]) -> String {
    runs.iter()
        .filter(|r| r.page == cell.page && run_center_inside(r, cell))
        .map(|r| r.text.as_str())
        .collect::<Vec<_>>()
        .join(" ")
        .trim()
        .to_string()
}

fn run_center_inside(run: &editor::TextRun, cell: &boxes::DetectedBox) -> bool {
    let cx = run.x + run.width / 2.0;
    let cy = run.y + run.height / 2.0;
    cx >= cell.x && cx <= cell.x + cell.width && cy >= cell.y && cy <= cell.y + cell.height
}

#[cfg(test)]
mod tests {
    use super::*;
    use pdfree_core::boxes::DetectedBox;
    use pdfree_core::editor::TextRun;

    fn cell(x: f32, y: f32) -> DetectedBox {
        DetectedBox {
            page: 0,
            x,
            y,
            width: 40.0,
            height: 20.0,
        }
    }

    fn run(text: &str, x: f32, y: f32) -> TextRun {
        TextRun {
            page: 0,
            text: text.to_string(),
            font_name: "Helvetica".to_string(),
            font_size: 10.0,
            x,
            y,
            width: 10.0,
            height: 8.0,
        }
    }

    #[test]
    fn too_few_cells_is_not_a_table() {
        let cells = vec![cell(0.0, 0.0), cell(50.0, 0.0), cell(0.0, 30.0)];
        assert!(table_from_cells(&cells, &[]).is_none());
    }

    #[test]
    fn a_single_row_is_not_a_table() {
        // Four cells but all on one row — no grid, just a row of boxes.
        let cells = vec![
            cell(0.0, 0.0),
            cell(50.0, 0.0),
            cell(100.0, 0.0),
            cell(150.0, 0.0),
        ];
        assert!(table_from_cells(&cells, &[]).is_none());
    }

    #[test]
    fn clusters_a_2x2_grid_into_rows_and_reads_cell_text() {
        // Two rows, two columns: (0,30) (50,30)
        //                        (0,0)  (50,0)
        let cells = vec![
            cell(0.0, 30.0),
            cell(50.0, 30.0),
            cell(0.0, 0.0),
            cell(50.0, 0.0),
        ];
        let runs = vec![
            run("Name", 5.0, 35.0),
            run("Age", 55.0, 35.0),
            run("Jane", 5.0, 5.0),
            run("30", 55.0, 5.0),
        ];
        let table = table_from_cells(&cells, &runs).unwrap();
        assert_eq!(
            table,
            vec![
                vec!["Name".to_string(), "Age".to_string()],
                vec!["Jane".to_string(), "30".to_string()],
            ]
        );
    }

    #[test]
    fn a_blank_cell_is_an_empty_string_not_an_error() {
        let cells = vec![
            cell(0.0, 30.0),
            cell(50.0, 30.0),
            cell(0.0, 0.0),
            cell(50.0, 0.0),
        ];
        let runs = vec![run("Header", 5.0, 35.0)];
        let table = table_from_cells(&cells, &runs).unwrap();
        assert_eq!(table[0], vec!["Header".to_string(), String::new()]);
        assert_eq!(table[1], vec![String::new(), String::new()]);
    }

    /// Real end-to-end pass against the IRS 1040 fixture, the same
    /// real-world form `pdfree-core`'s own `boxes.rs` tests were verified
    /// against for the exact same lattice-detection code this reuses.
    /// Doesn't assert on exact cell text (real-world OCR-adjacent text
    /// layout varies in whitespace) — just that at least one genuine grid
    /// (2+ rows, 2+ columns) is found somewhere in the document.
    #[test]
    fn extracts_at_least_one_real_table_from_a_real_form() {
        if pdfree_core::pdfium::bind().is_err() {
            eprintln!("skipping: PDFium library not found — run scripts/fetch-pdfium.sh to enable");
            return;
        }
        let fixture = include_bytes!("../../pdfree-core/tests/fixtures/irs_f1040.pdf");
        let tables = extract_tables(fixture).unwrap();
        assert!(!tables.is_empty(), "expected at least one detected table");
        for table in &tables {
            // Real-world grids are often ragged (merged cells, single-
            // column continuation rows) — `table_from_cells` already
            // guarantees at least 2 rows and one row with 2+ columns; don't
            // over-assert uniform column counts on top of that.
            assert!(table.len() >= 2);
            assert!(table.iter().any(|row| row.len() >= 2));
        }
    }
}
