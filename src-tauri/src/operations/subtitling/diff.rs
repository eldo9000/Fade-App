//! diff_subtitle — line-oriented diff between two subtitle files.
//!
//! Keeps dependencies minimal: we implement a small LCS-based diff rather
//! than pulling in the `similar` crate. Result is a list of entries tagged
//! equal/insert/delete/change, suitable for rendering as a side-by-side or
//! inline view in the Svelte analyze tab.

use serde::Serialize;

#[derive(Serialize, Clone)]
pub struct SubDiffLine {
    pub kind: String, // "eq" | "add" | "del"
    pub left: Option<String>,
    pub right: Option<String>,
}

fn lcs_table(a: &[&str], b: &[&str]) -> Vec<Vec<u32>> {
    let mut t = vec![vec![0u32; b.len() + 1]; a.len() + 1];
    for i in 1..=a.len() {
        for j in 1..=b.len() {
            t[i][j] = if a[i - 1] == b[j - 1] {
                t[i - 1][j - 1] + 1
            } else {
                t[i - 1][j].max(t[i][j - 1])
            };
        }
    }
    t
}

#[tauri::command]
pub async fn diff_subtitle(a_path: String, b_path: String) -> Result<Vec<SubDiffLine>, String> {
    tokio::task::spawn_blocking(move || -> Result<Vec<SubDiffLine>, String> {
        crate::validate_no_traversal(&a_path).map_err(|e| format!("a: {e}"))?;
        crate::validate_no_traversal(&b_path).map_err(|e| format!("b: {e}"))?;
        let a_body = super::read_subtitle_capped(&a_path).map_err(|e| format!("a: {e}"))?;
        let b_body = super::read_subtitle_capped(&b_path).map_err(|e| format!("b: {e}"))?;
        let a: Vec<&str> = a_body.lines().collect();
        let b: Vec<&str> = b_body.lines().collect();
        let t = lcs_table(&a, &b);

        // Backtrack.
        let mut out: Vec<SubDiffLine> = Vec::new();
        let mut i = a.len();
        let mut j = b.len();
        while i > 0 && j > 0 {
            if a[i - 1] == b[j - 1] {
                out.push(SubDiffLine {
                    kind: "eq".to_string(),
                    left: Some(a[i - 1].to_string()),
                    right: Some(b[j - 1].to_string()),
                });
                i -= 1;
                j -= 1;
            } else if t[i - 1][j] >= t[i][j - 1] {
                out.push(SubDiffLine {
                    kind: "del".to_string(),
                    left: Some(a[i - 1].to_string()),
                    right: None,
                });
                i -= 1;
            } else {
                out.push(SubDiffLine {
                    kind: "add".to_string(),
                    left: None,
                    right: Some(b[j - 1].to_string()),
                });
                j -= 1;
            }
        }
        while i > 0 {
            out.push(SubDiffLine {
                kind: "del".to_string(),
                left: Some(a[i - 1].to_string()),
                right: None,
            });
            i -= 1;
        }
        while j > 0 {
            out.push(SubDiffLine {
                kind: "add".to_string(),
                left: None,
                right: Some(b[j - 1].to_string()),
            });
            j -= 1;
        }
        out.reverse();
        Ok(out)
    })
    .await
    .map_err(|e| e.to_string())?
}
