//! Document diff / redline: compare two versions of a PDF, page-aligned,
//! word-level (Phase 7).
//!
//! Purely textual/geometric — no LLM call, same "specialized extractor, not
//! an LLM" tradeoff as `extract.rs`'s table extraction. Diffing is a classic
//! word-level LCS (longest-common-subsequence) dynamic program rather than a
//! dedicated diff crate: page-sized text (a few hundred to a couple thousand
//! words) is comfortably within the O(n*m) DP's practical range, and this
//! avoids pulling in a new dependency for something this codebase can
//! implement directly and test against a real document.
//!
//! Page alignment is by index (`pdfree_core::convert::to_text_per_page`,
//! not `to_text`'s joined string) — a version with pages inserted/removed in
//! the middle will show every following page as fully changed, since there's
//! no cross-document page-matching step. That's a known scope boundary: page
//! *reordering* detection would need something closer to a page-level
//! similarity match before the word-level diff runs, not attempted here.

use crate::Result;
use pdfree_core::convert;

/// What kind of change a [`TextChange`] represents.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChangeKind {
    /// Present, identical, in both versions.
    Unchanged,
    /// Present only in the new version.
    Added,
    /// Present only in the old version.
    Removed,
}

/// One contiguous run of same-kind words, in reading order relative to the
/// rest of the page's changes — concatenating every `TextChange.text` on a
/// page, in order, reconstructs the *new* version's page text (skip
/// `Removed` runs to do so; skip `Added` runs to reconstruct the *old*
/// version instead).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextChange {
    pub kind: ChangeKind,
    pub text: String,
}

/// The changes found on one page.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PageDiff {
    /// 0-based page index in the *new* document's page numbering (also
    /// valid against the old document except past the shorter document's
    /// last page — see the module docs on page alignment).
    pub page: u16,
    pub changes: Vec<TextChange>,
}

/// Above this many words on either side of a page pair, skip the word-level
/// LCS DP and fall back to a single whole-page Removed/Added (or Unchanged)
/// pair. The DP table is O(n*m) in both time and memory — at this cap it's
/// already ~16 MB for one page pair; an OCR-dense or programmatically
/// generated outlier page with tens of thousands of words would blow that up
/// quadratically for a feature meant to be a fast, in-process diff rather
/// than a batch job.
const MAX_WORDS_FOR_FINE_DIFF: usize = 2000;

/// Diff two versions of a PDF, page by page.
///
/// # Errors
///
/// Propagates `PDFium` / load errors from either document.
pub fn diff_documents(old_bytes: &[u8], new_bytes: &[u8]) -> Result<Vec<PageDiff>> {
    let old_pages = convert::to_text_per_page(old_bytes)?;
    let new_pages = convert::to_text_per_page(new_bytes)?;

    let page_count = old_pages.len().max(new_pages.len());
    let mut out = Vec::with_capacity(page_count);
    for i in 0..page_count {
        let old_text = old_pages.get(i).map_or("", String::as_str);
        let new_text = new_pages.get(i).map_or("", String::as_str);
        // Page counts are u16 throughout this crate; `page_count` is bounded
        // by two real documents' page counts, so this never truncates.
        #[allow(clippy::cast_possible_truncation)]
        let page = i as u16;
        out.push(PageDiff {
            page,
            changes: diff_page(old_text, new_text),
        });
    }
    Ok(out)
}

fn diff_page(old_text: &str, new_text: &str) -> Vec<TextChange> {
    let old_words: Vec<&str> = old_text.split_whitespace().collect();
    let new_words: Vec<&str> = new_text.split_whitespace().collect();

    if old_words.len() > MAX_WORDS_FOR_FINE_DIFF || new_words.len() > MAX_WORDS_FOR_FINE_DIFF {
        return coarse_diff(old_text, new_text);
    }

    group_changes(lcs_diff(&old_words, &new_words))
}

/// Whole-page Removed/Added (or a single Unchanged run if identical) —
/// see [`MAX_WORDS_FOR_FINE_DIFF`].
fn coarse_diff(old_text: &str, new_text: &str) -> Vec<TextChange> {
    if old_text == new_text {
        return if old_text.is_empty() {
            Vec::new()
        } else {
            vec![TextChange {
                kind: ChangeKind::Unchanged,
                text: old_text.to_string(),
            }]
        };
    }
    let mut out = Vec::new();
    if !old_text.is_empty() {
        out.push(TextChange {
            kind: ChangeKind::Removed,
            text: old_text.to_string(),
        });
    }
    if !new_text.is_empty() {
        out.push(TextChange {
            kind: ChangeKind::Added,
            text: new_text.to_string(),
        });
    }
    out
}

/// Classic suffix-LCS dynamic program: `dp[i][j]` is the length of the
/// longest common subsequence of `old_words[i..]` and `new_words[j..]`.
/// Walking forward from `(0, 0)`, always following whichever neighbor cell
/// holds the larger LCS length, produces a minimal edit script.
fn lcs_diff(old_words: &[&str], new_words: &[&str]) -> Vec<(ChangeKind, String)> {
    let n = old_words.len();
    let m = new_words.len();
    let mut dp = vec![vec![0u32; m + 1]; n + 1];
    for i in (0..n).rev() {
        for j in (0..m).rev() {
            dp[i][j] = if old_words[i] == new_words[j] {
                dp[i + 1][j + 1] + 1
            } else {
                dp[i + 1][j].max(dp[i][j + 1])
            };
        }
    }

    let mut out = Vec::new();
    let (mut i, mut j) = (0, 0);
    while i < n && j < m {
        if old_words[i] == new_words[j] {
            out.push((ChangeKind::Unchanged, old_words[i].to_string()));
            i += 1;
            j += 1;
        } else if dp[i + 1][j] >= dp[i][j + 1] {
            out.push((ChangeKind::Removed, old_words[i].to_string()));
            i += 1;
        } else {
            out.push((ChangeKind::Added, new_words[j].to_string()));
            j += 1;
        }
    }
    while i < n {
        out.push((ChangeKind::Removed, old_words[i].to_string()));
        i += 1;
    }
    while j < m {
        out.push((ChangeKind::Added, new_words[j].to_string()));
        j += 1;
    }
    out
}

/// Collapse consecutive same-kind words into one space-joined [`TextChange`].
fn group_changes(words: Vec<(ChangeKind, String)>) -> Vec<TextChange> {
    let mut out: Vec<TextChange> = Vec::new();
    for (kind, word) in words {
        match out.last_mut() {
            Some(last) if last.kind == kind => {
                last.text.push(' ');
                last.text.push_str(&word);
            }
            _ => out.push(TextChange { kind, text: word }),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identical_pages_are_one_unchanged_run() {
        let changes = diff_page("the quick brown fox", "the quick brown fox");
        assert_eq!(
            changes,
            vec![TextChange {
                kind: ChangeKind::Unchanged,
                text: "the quick brown fox".to_string(),
            }]
        );
    }

    #[test]
    fn two_blank_pages_produce_no_changes() {
        assert!(diff_page("", "").is_empty());
    }

    #[test]
    fn a_replaced_word_is_one_removed_and_one_added_run() {
        let changes = diff_page("the cat sat", "the dog sat");
        assert_eq!(
            changes,
            vec![
                TextChange {
                    kind: ChangeKind::Unchanged,
                    text: "the".to_string()
                },
                TextChange {
                    kind: ChangeKind::Removed,
                    text: "cat".to_string()
                },
                TextChange {
                    kind: ChangeKind::Added,
                    text: "dog".to_string()
                },
                TextChange {
                    kind: ChangeKind::Unchanged,
                    text: "sat".to_string()
                },
            ]
        );
    }

    #[test]
    fn an_inserted_word_is_added_only() {
        let changes = diff_page("the cat sat", "the cat sat quietly");
        assert_eq!(changes.last().unwrap().kind, ChangeKind::Added);
        assert_eq!(changes.last().unwrap().text, "quietly");
        assert!(changes.iter().all(|c| c.kind != ChangeKind::Removed));
    }

    #[test]
    fn a_deleted_word_is_removed_only() {
        let changes = diff_page("the cat sat quietly", "the cat sat");
        assert_eq!(changes.last().unwrap().kind, ChangeKind::Removed);
        assert_eq!(changes.last().unwrap().text, "quietly");
        assert!(changes.iter().all(|c| c.kind != ChangeKind::Added));
    }

    #[test]
    fn a_page_added_past_the_old_documents_length_is_fully_added() {
        // Mirrors how diff_documents treats a page index past the shorter
        // document's page count: old_text is "".
        let changes = diff_page("", "a whole new page of content");
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].kind, ChangeKind::Added);
        assert_eq!(changes[0].text, "a whole new page of content");
    }

    #[test]
    fn a_page_removed_past_the_new_documents_length_is_fully_removed() {
        let changes = diff_page("a whole page that got deleted", "");
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].kind, ChangeKind::Removed);
    }

    #[test]
    fn oversized_pages_fall_back_to_coarse_whole_page_diff() {
        let big_old = (0..MAX_WORDS_FOR_FINE_DIFF + 1)
            .map(|i| format!("w{i}"))
            .collect::<Vec<_>>()
            .join(" ");
        let big_new = format!("{big_old} extra");
        let changes = diff_page(&big_old, &big_new);
        // Coarse fallback: one Removed (the whole old page) + one Added
        // (the whole new page), not a per-word LCS breakdown.
        assert_eq!(changes.len(), 2);
        assert_eq!(changes[0].kind, ChangeKind::Removed);
        assert_eq!(changes[1].kind, ChangeKind::Added);
    }

    #[test]
    fn oversized_identical_pages_are_a_single_coarse_unchanged_run() {
        let big = (0..MAX_WORDS_FOR_FINE_DIFF + 1)
            .map(|i| format!("w{i}"))
            .collect::<Vec<_>>()
            .join(" ");
        let changes = diff_page(&big, &big);
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].kind, ChangeKind::Unchanged);
    }

    /// Real end-to-end pass: overlay a distinctive new string onto a copy of
    /// a genuine IRS form, then diff the original against the modified copy
    /// via real `PDFium` text extraction (not hand-built strings, like the
    /// unit tests above).
    #[test]
    fn diff_documents_detects_a_real_stamped_addition() {
        let original = include_bytes!("../../pdfree-core/tests/fixtures/irs_f1040.pdf");
        if pdfree_core::pdfium::bind().is_err() {
            eprintln!("skipping: PDFium library not found — run scripts/fetch-pdfium.sh to enable");
            return;
        }

        let modified = pdfree_core::forms::overlay_text(
            original,
            &[pdfree_core::forms::TextOverlay {
                page: 0,
                x: 50.0,
                y: 700.0,
                text: "ZZZDIFFTESTZZZ".to_string(),
                font_size: 12.0,
            }],
        )
        .unwrap();

        let diffs = diff_documents(original, &modified).unwrap();
        let page0 = diffs.iter().find(|d| d.page == 0).unwrap();
        assert!(
            page0
                .changes
                .iter()
                .any(|c| c.kind == ChangeKind::Added && c.text.contains("ZZZDIFFTESTZZZ")),
            "expected an Added change containing the stamped text on page 0"
        );

        // Every other page should be untouched (well within the fine-diff
        // path, since the fixture's real pages are far under the word cap).
        for other in diffs.iter().filter(|d| d.page != 0) {
            assert!(
                other
                    .changes
                    .iter()
                    .all(|c| c.kind == ChangeKind::Unchanged),
                "page {} unexpectedly changed",
                other.page
            );
        }
    }
}
