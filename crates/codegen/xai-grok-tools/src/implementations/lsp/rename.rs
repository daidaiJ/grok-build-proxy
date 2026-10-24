//! Symbol rename (`textDocument/rename` → WorkspaceEdit → files).
//!
//! The protocol half lives in the adapter; this module holds the pure part:
//! flattening a [`WorkspaceEdit`] into per-file edit lists, and applying a
//! list of [`TextEdit`]s to a file's current text with the checks that make a
//! stale server answer fail loudly instead of corrupting the file — sorted,
//! bounds-checked, overlap-rejected (the qwen lesson: never trust offsets
//! computed against text we have not verified we hold).

use async_lsp::lsp_types::{TextEdit, Url, WorkspaceEdit};

/// Flatten a WorkspaceEdit into `(uri, edits)` pairs. `documentChanges` takes
/// precedence over `changes` when a server sends both (LSP semantics). File
/// resource operations (`create`/`rename`/`delete` inside `documentChanges`)
/// are rejected — a symbol rename must not delete files as a side effect.
pub fn workspace_edit_to_file_edits(
    edit: WorkspaceEdit,
) -> Result<Vec<(Url, Vec<TextEdit>)>, String> {
    if let Some(document_changes) = edit.document_changes {
        let mut out = Vec::new();
        let mut push_doc_edit = |doc_edit: async_lsp::lsp_types::TextDocumentEdit| {
            let text_edits = doc_edit
                .edits
                .into_iter()
                .map(|one_of| match one_of {
                    async_lsp::lsp_types::OneOf::Left(edit) => edit,
                    async_lsp::lsp_types::OneOf::Right(annotated) => annotated.text_edit,
                })
                .collect();
            out.push((doc_edit.text_document.uri, text_edits));
        };
        match document_changes {
            async_lsp::lsp_types::DocumentChanges::Edits(edits) => {
                for doc_edit in edits {
                    push_doc_edit(doc_edit);
                }
            }
            async_lsp::lsp_types::DocumentChanges::Operations(ops) => {
                for op in ops {
                    match op {
                        async_lsp::lsp_types::DocumentChangeOperation::Edit(doc_edit) => {
                            push_doc_edit(doc_edit);
                        }
                        async_lsp::lsp_types::DocumentChangeOperation::Op(_) => {
                            return Err(
                                "the server returned a file operation (create/rename/delete) \
                                 for this symbol rename; refusing to apply it"
                                    .into(),
                            );
                        }
                    }
                }
            }
        }
        return Ok(out);
    }
    if let Some(changes) = edit.changes {
        return Ok(changes.into_iter().collect());
    }
    Ok(Vec::new())
}

/// Byte offset of an LSP position (line, UTF-16 character) in `content`.
/// `Err` when the position falls past the end of the text — the server's
/// edit was computed against a version we do not hold.
fn position_to_offset(content: &str, line: u32, character: u32) -> Result<usize, String> {
    let mut current_line = 0u32;
    let mut line_start = 0usize;
    for (idx, ch) in content.char_indices() {
        if current_line == line {
            break;
        }
        if ch == '\n' {
            current_line += 1;
            line_start = idx + 1;
        }
    }
    if current_line != line {
        return Err(format!("line {line} is past the end of the file"));
    }
    let line_rest = &content[line_start..];
    let line_end = line_rest.find('\n').map_or(line_rest.len(), |idx| idx);
    let mut utf16_units = 0u32;
    for (idx, ch) in line_rest[..line_end].char_indices() {
        if utf16_units >= character {
            return Ok(line_start + idx);
        }
        utf16_units += ch.len_utf16() as u32;
    }
    if utf16_units < character {
        return Err(format!(
            "column {character} is past the end of line {line}"
        ));
    }
    Ok(line_start + line_end)
}

/// Apply `edits` to `content`. Edits are validated (in bounds, no overlap)
/// and applied from the end of the file backwards, so earlier offsets stay
/// valid. Any violation rejects the whole batch — a half-applied rename is
/// worse than none.
pub fn apply_text_edits(content: &str, edits: &[TextEdit]) -> Result<String, String> {
    if edits.is_empty() {
        return Ok(content.to_string());
    }
    let mut sorted: Vec<&TextEdit> = edits.iter().collect();
    sorted.sort_by(|a, b| {
        (a.range.start.line, a.range.start.character)
            .cmp(&(b.range.start.line, b.range.start.character))
    });
    // Overlap check in document order: each edit must start at or after the
    // previous one ends.
    let mut previous_end: Option<(u32, u32)> = None;
    for edit in &sorted {
        if edit.range.end.line < edit.range.start.line
            || (edit.range.end.line == edit.range.start.line
                && edit.range.end.character < edit.range.start.character)
        {
            return Err("edit range ends before it starts".into());
        }
        if let Some((end_line, end_char)) = previous_end
            && (edit.range.start.line < end_line
                || (edit.range.start.line == end_line && edit.range.start.character < end_char))
        {
            return Err("overlapping edits from the server; refusing to apply".into());
        }
        previous_end = Some((edit.range.end.line, edit.range.end.character));
    }

    let mut offsets: Vec<(usize, usize, &str)> = Vec::with_capacity(sorted.len());
    for edit in &sorted {
        let start = position_to_offset(content, edit.range.start.line, edit.range.start.character)?;
        let end = position_to_offset(content, edit.range.end.line, edit.range.end.character)?;
        offsets.push((start, end, edit.new_text.as_str()));
    }
    let mut result = content.to_string();
    // Backwards, so earlier (start, end) offsets stay valid.
    for (start, end, new_text) in offsets.into_iter().rev() {
        result.replace_range(start..end, new_text);
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_lsp::lsp_types::{Position, Range};

    fn edit(start_line: u32, start_char: u32, end_line: u32, end_char: u32, new_text: &str) -> TextEdit {
        TextEdit {
            range: Range {
                start: Position { line: start_line, character: start_char },
                end: Position { line: end_line, character: end_char },
            },
            new_text: new_text.to_string(),
        }
    }

    #[test]
    fn edits_apply_in_document_order_regardless_of_input_order() {
        let content = "alpha\nbeta\ngamma\n";
        let edits = vec![edit(2, 0, 2, 5, "GAMMA"), edit(0, 0, 0, 5, "ALPHA")];
        assert_eq!(
            apply_text_edits(content, &edits).unwrap(),
            "ALPHA\nbeta\nGAMMA\n"
        );
    }

    #[test]
    fn an_insert_between_two_edits_keeps_both() {
        let content = "one two";
        let edits = vec![edit(0, 3, 0, 3, "-"), edit(0, 0, 0, 3, "ONE")];
        assert_eq!(apply_text_edits(content, &edits).unwrap(), "ONE- two");
    }

    #[test]
    fn an_out_of_bounds_edit_rejects_the_batch() {
        let content = "short\n";
        let edits = vec![edit(5, 0, 5, 3, "x")];
        assert!(apply_text_edits(content, &edits).is_err());
    }

    #[test]
    fn an_overlapping_pair_rejects_the_batch() {
        let content = "abcdef";
        let edits = vec![edit(0, 0, 0, 4, "x"), edit(0, 2, 0, 6, "y")];
        assert!(apply_text_edits(content, &edits).is_err());
    }

    #[test]
    fn utf16_columns_count_surrogate_pairs_as_two() {
        let content = "a🚀b";
        // Replace the rocket (utf16 columns 1..3) with 'r'.
        let edits = vec![edit(0, 1, 0, 3, "r")];
        assert_eq!(apply_text_edits(content, &edits).unwrap(), "arb");
    }

    #[test]
    fn empty_edit_list_is_a_no_op() {
        assert_eq!(apply_text_edits("same", &[]).unwrap(), "same");
    }
}
