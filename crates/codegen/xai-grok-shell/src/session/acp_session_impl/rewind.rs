//! Rewind concern for `SessionActor`: rewind points, cross-compaction replay detection, and `handle_rewind`.

use super::*;

/// First non-empty line of the prompt, truncated to 60 chars — the picker preview.
fn preview_of(text: &str) -> Option<String> {
    let clean_text = extract_user_query(text);
    let first_line = clean_text
        .lines()
        .map(|l| l.trim())
        .find(|l| !l.is_empty())
        .unwrap_or("");

    if first_line.is_empty() {
        None
    } else if first_line.chars().count() > 60 {
        Some(format!("{}...", crate::util::truncate(first_line, 57)))
    } else {
        Some(first_line.to_string())
    }
}

impl SessionActor {
    pub(super) async fn close_rewind_window(&self) {
        let mut state = self.state.lock().await;
        state.rewindable = false;
    }

    /// Returns the `prompt_index → num_file_snapshots` map from the on-disk snapshot index (independent of the chat-state prompt index).
    /// The bridge joins these onto the server's rewind points.
    pub(super) async fn rewind_file_counts(&self) -> std::collections::HashMap<usize, usize> {
        self.file_state_tracker
            .get_rewind_point_metas()
            .await
            .into_iter()
            .map(|m| (m.prompt_index, m.num_file_snapshots))
            .collect()
    }

    /// Get available rewind points for this session.
    /// Every prompt is a checkpoint: the list always contains `[0, 1, ..., N-1]` where N is the current prompt_index.
    /// File snapshots may or may not exist for each checkpoint (indicated by `has_file_changes`).
    /// LOCAL (branch-tree undo): points additionally carry their branch coordinate and a
    /// pre-computed replay boundary; the response lists abandoned branches for redo.
    pub(super) async fn get_rewind_points(&self) -> RewindPointsResponse {
        // Metadata only: don't load the (huge) file-content snapshots just to render the picker
        let file_metas = self.file_state_tracker.get_rewind_point_metas().await;

        // Query prompt state from the chat state actor.
        let snapshot = self.chat_state_handle.snapshot().await;
        let (prompts, current_prompt_index) = match snapshot {
            Some(ref s) => (s.prompt_texts.clone(), s.prompt_index),
            None => (vec![], 0),
        };

        // Build a lookup of which prompt indices have file snapshots.
        let file_meta_map: std::collections::HashMap<
            usize,
            &xai_grok_workspace::session::file_state::RewindPointMeta,
        > = file_metas.iter().map(|m| (m.prompt_index, m)).collect();

        // Generate a rewind point for every prompt 0..current_prompt_index.
        let mut rewind_points = (0..current_prompt_index)
            .map(|idx| {
                let prompt_preview = prompts.get(idx).and_then(|text| preview_of(text));

                let file_meta = file_meta_map.get(&idx);
                let num_file_snapshots = file_meta.map_or(0, |m| m.num_file_snapshots);
                let created_at = file_meta
                    .map(|m| m.created_at.to_rfc3339())
                    .unwrap_or_default();

                RewindPointInfo {
                    prompt_index: idx,
                    created_at,
                    num_file_snapshots,
                    has_file_changes: num_file_snapshots > 0,
                    prompt_preview,
                    branch: 0,
                    boundary: None,
                }
            })
            .collect::<Vec<_>>();

        let mut abandoned_branches: Vec<RewindBranchInfo> = Vec::new();

        // LOCAL (branch-tree undo): derive the journal branch face — branch coordinates
        // for each timeline point, per-point replay boundary, and the abandoned branches
        // the redo path can switch back to.
        let session_dir = crate::session::persistence::session_dir(&self.session_info);
        let updates_path = session_dir.join("updates.jsonl");
        if let Ok(Some(iter)) = crate::session::storage::PromptExtractIterator::open(&updates_path)
        {
            let face = crate::session::storage::collect_rewind_face(iter);
            let checkpoints = crate::session::storage::collect_branch_checkpoints(&updates_path)
                .unwrap_or_default();
            let slices = face.active_chain_slices();
            let chain_branches: std::collections::HashSet<usize> =
                slices.iter().map(|(b, _, _)| *b).collect();

            // Map each timeline point to its (branch, own index) coordinate.
            let mut coords: Vec<(u64, usize)> =
                vec![(0, 0); rewind_points.len().max(1)];
            for (branch, kept, base) in &slices {
                for i in 0..*kept {
                    if let Some(c) = coords.get_mut(base + i) {
                        *c = (*branch as u64, i);
                    }
                }
            }

            // Chain checkpoints, normalized to active-timeline coordinates.
            let mut chain_ckpts: Vec<(usize, &str)> = checkpoints
                .iter()
                .filter(|c| chain_branches.contains(&c.branch))
                .filter_map(|c| {
                    let (branch, _kept, base) =
                        slices.iter().copied().find(|(b, _, _)| *b == c.branch)?;
                    let t = face.branches[branch].fork.map_or(0, |(_, t)| t);
                    (c.at >= t).then(|| (base + (c.at - t), c.file.as_str()))
                })
                .collect();
            chain_ckpts.sort_by_key(|(at, _)| *at);

            for (i, point) in rewind_points.iter_mut().enumerate() {
                if let Some(c) = coords.get(i) {
                    point.branch = c.0;
                }
                // Boundary precompute (kimi ForkLineError): the point fails iff the
                // innermost surviving checkpoint base at its target is unreadable.
                if let Some((_, file)) =
                    chain_ckpts.iter().filter(|(at, _)| *at <= i).last()
                    && !session_dir.join(file).exists()
                {
                    point.boundary = Some(format!(
                        "compaction checkpoint {file} is gone — rewinding here would lose history; pick a later point"
                    ));
                }
            }

            // Abandoned branches: everything off the active chain, points 0..=tip.
            for (b, node) in face.branches.iter().enumerate() {
                if chain_branches.contains(&b) {
                    continue;
                }
                let points = (0..=node.prompts.len())
                    .map(|own| RewindPointInfo {
                        prompt_index: own,
                        created_at: String::new(),
                        num_file_snapshots: 0,
                        has_file_changes: false,
                        prompt_preview: node.prompts.get(own).and_then(|t| preview_of(t)),
                        branch: b as u64,
                        boundary: None,
                    })
                    .collect();
                abandoned_branches.push(RewindBranchInfo {
                    branch: b as u64,
                    fork: node.fork.map(|(p, t)| (p as u64, t)),
                    points,
                });
            }
        }

        RewindPointsResponse {
            rewind_points,
            abandoned_branches,
        }
    }

    /// Load user prompts from `updates.jsonl` in chronological order.
    /// `RewindMarker` entries truncate the list back to the marker's target so only prompts from the current timeline are returned.
    /// Uses [`PromptExtractIterator`] which peeks at the `update.sessionUpdate` discriminant field without fully deserialising every notification.
    pub(super) fn load_user_prompts_from_updates(
        updates_path: &std::path::Path,
    ) -> std::io::Result<Vec<String>> {
        use crate::session::storage::{PromptExtractIterator, collect_prompts_from_events};

        let Some(iter) = PromptExtractIterator::open(updates_path)? else {
            return Ok(vec![]);
        };

        tracing::debug!(
            path = %updates_path.display(),
            "load_user_prompts_from_updates: starting selective scan"
        );

        let prompts = collect_prompts_from_events(iter);

        tracing::debug!(
            prompt_count = prompts.len(),
            "load_user_prompts_from_updates: done"
        );

        Ok(prompts)
    }

    /// Check whether a rewind must replay `updates.jsonl` to reconstruct the conversation: replay whenever a compaction has occurred.
    /// Compaction collapses N+1 user messages into ~3, so the conversation in memory no longer has the User count `prompt_index` implies.
    /// `truncate_to_prompt_index` counts User items to find the cut point, so it is wrong for ALL post-compaction targets, not just at the boundary.
    /// "Rewind to N" restores the state from before prompt N ran; prompts 0..N-1 are kept.
    /// `All`: roll back both conversation and files.
    /// `ConversationOnly`: roll back conversation, leave files untouched.
    pub(super) async fn handle_rewind(
        &self,
        request: RewindRequest,
    ) -> anyhow::Result<RewindResponse> {
        // LOCAL (branch-tree undo): the redo form switches back to an abandoned branch
        // instead of truncating the current one — completely different validation and
        // rebuild path, so it gets its own handler.
        if let Some(to_branch) = request.to_branch {
            return self.handle_rewind_to_branch(request, to_branch).await;
        }

        self.signals_handle().mark_reverted();

        let target_index = request.target_prompt_index;
        let mode = request.mode;
        let wants_file_revert = matches!(mode, RewindMode::All | RewindMode::FilesOnly);
        let wants_conversation_rewind =
            matches!(mode, RewindMode::All | RewindMode::ConversationOnly);
        let _strip_guard = if request.force && wants_conversation_rewind {
            Some(self.prepare_image_strips_for_rewind().await)
        } else {
            None
        };

        // Validate: target must be less than current prompt_index
        // FilesOnly reverts the on-disk snapshot index (bounded by `get_rewind_points`, not the conversation), so it is exempt
        // In bridge mode the conversation lives server-side and the chat-state prompt index is empty
        let current_prompt_index = self.chat_state_handle.get_prompt_index().await;
        if mode != RewindMode::FilesOnly && target_index >= current_prompt_index {
            return Ok(RewindResponse {
                success: false,
                target_prompt_index: target_index,
                mode,
                reverted_files: vec![],
                clean_files: vec![],
                conflicts: vec![],
                prompt_text: None,
                error: Some(format!(
                    "Cannot rewind to prompt #{} — current prompt index is {}. \
                     Valid targets: 0..{}",
                    target_index,
                    current_prompt_index,
                    current_prompt_index.saturating_sub(1)
                )),
            });
        }

        // ── Build file revert preview (for All and FilesOnly modes) ─────
        let mut clean_files = Vec::new();
        let mut conflicts = Vec::new();

        // Collect files that would be reverted and detect conflicts; this is read-only
        let mut files_to_revert: std::collections::HashMap<
            xai_grok_workspace::session::file_state::FlexiblePath,
            Option<String>,
        > = std::collections::HashMap::new();

        if wants_file_revert {
            let all_points = self.file_state_tracker.get_rewind_points().await;

            for point in all_points.iter().filter(|p| p.prompt_index >= target_index) {
                for (path, before_snapshot) in &point.file_snapshots {
                    // Only keep the earliest snapshot for each file
                    files_to_revert
                        .entry(path.clone())
                        .or_insert_with(|| before_snapshot.content.clone());
                }
            }

            // Build conflict/clean lists for the preview
            for path in files_to_revert.keys() {
                let current_content = self
                    .tool_context
                    .fs
                    .try_read_to_string(path)
                    .await
                    .unwrap_or(None);

                // Find the latest after_snapshot for this file (what the agent most recently left it as) for conflict detection
                let after_content = all_points
                    .iter()
                    .rev()
                    .find_map(|p| p.after_snapshots.get(path))
                    .and_then(|s| s.content.clone());

                let is_clean = current_content == after_content;

                if is_clean {
                    clean_files.push(path.to_string());
                } else {
                    let conflict_type = if current_content.is_none() && after_content.is_some() {
                        "deleted_externally"
                    } else if current_content.is_some() && after_content.is_none() {
                        "created_externally"
                    } else {
                        "modified_externally"
                    };
                    conflicts.push(RewindConflictInfo {
                        path: path.to_string(),
                        conflict_type: conflict_type.to_string(),
                    });
                }
            }
        }

        // ── Preview mode (force=false): pure dry run, no mutations ────
        // Return what WOULD happen so the TUI can show a confirmation modal
        if !request.force {
            let error = if !conflicts.is_empty() {
                Some("External modifications detected. Confirm to revert anyway.".to_string())
            } else {
                None
            };
            return Ok(RewindResponse {
                success: false,
                target_prompt_index: target_index,
                mode,
                reverted_files: vec![],
                clean_files,
                conflicts,
                prompt_text: None,
                error,
            });
        }

        // ── Commit mode (force=true): execute the rewind ─────────────

        // Execute file revert
        let mut reverted_files = Vec::new();
        if wants_file_revert {
            for (rel_path, content) in files_to_revert {
                match &content {
                    Some(data) => {
                        if let Err(e) = self
                            .tool_context
                            .fs
                            .write_file(&rel_path, data.as_bytes())
                            .await
                        {
                            tracing::warn!(?e, "Failed to restore file during rewind");
                            continue;
                        }
                    }
                    None => {
                        if self
                            .tool_context
                            .fs
                            .exists(&rel_path)
                            .await
                            .unwrap_or(false)
                            && let Err(e) = self.tool_context.fs.delete_file(&rel_path).await
                        {
                            tracing::warn!(?e, "Failed to delete file during rewind");
                        }
                    }
                }
                reverted_files.push(rel_path.to_string());
            }
        }

        // Execute conversation rewind
        let mut prompt_text: Option<String> = None;
        if wants_conversation_rewind {
            let session_dir = crate::session::persistence::session_dir(&self.session_info);
            let updates_path = session_dir.join("updates.jsonl");

            if let Some(snap) = self.chat_state_handle.snapshot().await {
                prompt_text = snap.prompt_texts.get(target_index).cloned();
            }

            // Store for edit-and-retry detection in the next prompt() call
            if let Ok(mut pending) = self.rewind_pending_prompt.lock() {
                *pending = prompt_text.clone();
            }

            // LOCAL (T2b): every rewind rebuilds through the journal replay — one path for
            // below- and above-compaction targets alike, so the rebuilt conversation always
            // matches what a resume would load (tool calls/results and image parts included)
            // instead of the old dual path (in-memory truncate above the compaction point,
            // text-only replay below it).
            let mut conversation = self.chat_state_handle.get_conversation().await;

            // Replay recomputes whether a compaction summary survives; `None` keeps the existing marker
            let replay_compaction_marker: Option<Option<usize>>;

            // Run on the blocking pool since replay does synchronous file I/O (reading checkpoint files and scanning updates.jsonl)
            let replay_updates = updates_path.clone();
            let replay_session_dir = session_dir.clone();
            let replay_target = target_index;
            let replay_result = tokio::task::spawn_blocking(move || {
                crate::session::helpers::replay::replay_to_prompt(
                    &replay_updates,
                    &replay_session_dir,
                    replay_target,
                )
            })
            .await
            .map_err(|e| anyhow::anyhow!("spawn_blocking panicked: {e}"))?;
            match replay_result {
                Ok(replay_result) => {
                    tracing::info!(
                        target_index,
                        prompt_index_reached = replay_result.prompt_index_reached,
                        conversation_len = replay_result.conversation.len(),
                        "Rewind: conversation reconstructed via replay"
                    );
                    // The rebuilt conversation drops the summary unless a checkpoint survived
                    // Carry the recomputed marker to the snapshot restore so the stale value isn't reused
                    replay_compaction_marker = Some(replay_result.last_compaction_prompt_index);
                    // The replay result may or may not include the session preamble (System and User(user_info)).
                    // Raw updates (target < compaction_at): replay only accumulates user/agent turns from updates.jsonl.
                    // Prepend System and the original User(user_info) so the model sees the same preamble it originally saw.
                    if matches!(
                        replay_result.conversation.first(),
                        Some(ConversationItem::System(_))
                    ) {
                        conversation = replay_result.conversation;
                    } else {
                        // Keep System (index 0)
                        // Replace User(user_info) at index 1 with the original from the checkpoint if available, otherwise keep the current one
                        if let Some(ui0) = replay_result.original_user_info {
                            conversation.truncate(1); // keep System only
                            conversation.push(ConversationItem::user(ui0));
                        } else {
                            conversation.truncate(2); // keep System + current user_info
                        }
                        conversation.extend(replay_result.conversation);
                    }
                }
                Err(e) => {
                    tracing::error!(
                        ?e,
                        target_index,
                        "Rewind replay failed — rewind aborted"
                    );
                    // No fallback to truncation: a rebuild failure means the journal could
                    // not answer for this target; falling back would silently pick a
                    // different reconstruction semantics.
                    return Ok(RewindResponse {
                        success: false,
                        target_prompt_index: target_index,
                        mode,
                        reverted_files: vec![],
                        clean_files: vec![],
                        conflicts: vec![],
                        prompt_text: None,
                        error: Some(format!("Cannot rewind to prompt #{target_index}: {e}")),
                    });
                }
            }

            self.cancel_active_sampling_requests();
            self.cancel_pending_image_strips_for_rewind();
            self.chat_state_handle.replace_conversation(conversation);
            // Use a snapshot to set the correct prompt_index and truncated prompt_texts.
            // The actor's TruncateToPromptIndex doesn't apply here because the conversation was already truncated locally
            // Instead, snapshot and restore with the corrected fields
            if let Some(mut snap) = self.chat_state_handle.snapshot().await {
                snap.prompt_index = target_index;
                snap.prompt_texts.truncate(target_index);
                // Cross-compaction rewind recomputes the marker (the rebuilt conversation may have dropped the summary)
                // Standard truncation keeps the existing marker
                let new_marker =
                    replay_compaction_marker.unwrap_or(snap.last_compaction_prompt_index);
                snap.last_compaction_prompt_index = new_marker;
                self.chat_state_handle.restore_snapshot(snap);
            }

            // The conversation shrank: clear budget-based (size/schema) and stale per-turn suppression so compaction can run on the smaller context
            // Account-state suppression (credit/auth sets SUPPRESS_UNTIL_SUCCESS) isn't budget-related, so it persists until a successful model call
            if self
                .compaction
                .auto_compact_suppressed
                .load(std::sync::atomic::Ordering::Relaxed)
                != crate::session::compaction_config::SUPPRESS_UNTIL_SUCCESS
            {
                self.compaction.auto_compact_suppressed.store(
                    crate::session::compaction_config::SUPPRESS_NONE,
                    std::sync::atomic::Ordering::Relaxed,
                );
            }

            // The rewind may have dropped failed-server reminders with the truncated turns, so still-down servers must re-announce
            // See rearm_failed_server_announcements for why connected fingerprints stay latched
            self.rearm_failed_server_announcements().await;

            // Append a RewindMarker to updates.jsonl so replay can handle a branched timeline (updates.jsonl is append-only)
            // LOCAL (branch-tree undo): `to_branch: None` = forward rewind; T2's redo path appends `Some(b)` instead.
            self.persist_xai_update_only(XaiSessionUpdate::RewindMarker {
                target_prompt_index: target_index,
                created_at: chrono::Utc::now().to_rfc3339(),
                to_branch: None,
            });

            // The turn summary and recap describe turns the rewind just removed
            // Abort in-flight side-calls and clear the persisted copies so session lists don't show stale work
            // Bumping the recap epoch stops an in-flight recap from committing (and re-persisting `last_recap`) after the clear below
            self.recap_epoch.set(self.recap_epoch.get().wrapping_add(1));
            self.abort_turn_summary();
            self.abort_title_refresh();
            let _ = self
                .notifications
                .persistence_tx
                .send(PersistenceMsg::LastTurnSummary(None));
            let _ = self
                .notifications
                .persistence_tx
                .send(PersistenceMsg::LastRecap(None));

            // Re-derive the AUTO title-refresh checkpoint from the shortened conversation.
            // A rewind below a checkpoint re-opens refreshing, while one still past the window stays frozen.
            // Persist it so the reopened state survives resume (unlike compaction, a rewind genuinely removes the turns those checkpoints described).
            let session_dir = crate::session::persistence::session_dir(&self.session_info);
            if !crate::session::persistence::title_is_manual_in_dir(&session_dir) {
                let post_rewind_turns = crate::session::helpers::session_recap::main_turn_count(
                    &self.chat_state_handle.get_conversation().await,
                );
                let idx = crate::session::helpers::session_summary::checkpoints_reached(
                    post_rewind_turns,
                );
                self.next_title_refresh_idx.set(idx);
                crate::session::helpers::session_summary::save_title_refresh_watermark(
                    &session_dir,
                    idx,
                );
            }
        }

        // Update the file state tracker to reflect the rewind.
        if wants_file_revert {
            // All/FilesOnly: files were reverted and the snapshots are now stale, so truncate them
            self.file_state_tracker.truncate_from(target_index).await;
            let _ = self
                .notifications
                .persistence_tx
                .send(PersistenceMsg::TruncateRewindPoints {
                    from_index: target_index,
                });
        } else if wants_conversation_rewind {
            // ConversationOnly: files are untouched but the conversation is rewound.
            self.merge_rewind_tracker_from(target_index).await;
        }

        Ok(RewindResponse {
            success: true,
            target_prompt_index: target_index,
            mode,
            reverted_files,
            clean_files: vec![],
            conflicts,
            prompt_text,
            error: None,
        })
    }

    /// LOCAL (branch-tree undo) redo: switch the active timeline back to abandoned
    /// branch `to_branch` at `request.target_prompt_index` (its own index). Appends a
    /// redo-form `RewindMarker` and rebuilds the conversation through the branch-aware
    /// replay, mirroring the cross-compaction path's splice and housekeeping. The file
    /// half is intentionally untouched (same semantics as kimi's undo; design §4).
    pub(super) async fn handle_rewind_to_branch(
        &self,
        request: RewindRequest,
        to_branch: u64,
    ) -> anyhow::Result<RewindResponse> {
        if request.mode != RewindMode::ConversationOnly {
            tracing::info!(
                to_branch,
                mode = ?request.mode,
                "redo rewind forces conversation-only semantics (file half not re-applied)"
            );
        }

        let target_index = request.target_prompt_index;
        let mode = request.mode;

        let session_dir = crate::session::persistence::session_dir(&self.session_info);
        let updates_path = session_dir.join("updates.jsonl");

        let error_response = |target: usize, error: String| RewindResponse {
            success: false,
            target_prompt_index: target,
            mode,
            reverted_files: vec![],
            clean_files: vec![],
            conflicts: vec![],
            prompt_text: None,
            error: Some(error),
        };

        // Validate the (branch, own index) point against the journal branch face.
        let face = match crate::session::storage::PromptExtractIterator::open(&updates_path) {
            Ok(Some(iter)) => crate::session::storage::collect_rewind_face(iter),
            _ => {
                return Ok(error_response(
                    target_index,
                    "No replayable session journal — cannot switch branches".to_string(),
                ));
            }
        };
        let Some(branch) = face.branches.get(to_branch as usize) else {
            return Ok(error_response(
                target_index,
                format!("Branch #{to_branch} does not exist in this session's journal"),
            ));
        };
        if target_index > branch.prompts.len() {
            return Ok(error_response(
                target_index,
                format!(
                    "Branch #{to_branch} has {} prompts — cannot rewind to #{}",
                    branch.prompts.len(),
                    target_index
                ),
            ));
        }
        let timeline_target = branch.fork.map_or(0, |(_, t)| t) + target_index;
        let prompt_text = branch.prompts.get(target_index).cloned();

        // Preview mode: hand back the prompt text so the pager confirm modal can offer
        // the kimi-style "edit and resend" flow.
        if !request.force {
            return Ok(RewindResponse {
                success: false,
                target_prompt_index: timeline_target,
                mode,
                reverted_files: vec![],
                clean_files: vec![],
                conflicts: vec![],
                prompt_text,
                error: None,
            });
        }

        self.signals_handle().mark_reverted();
        let _strip_guard = if request.force {
            Some(self.prepare_image_strips_for_rewind().await)
        } else {
            None
        };

        // Store for edit-and-retry detection in the next prompt() call.
        if let Ok(mut pending) = self.rewind_pending_prompt.lock() {
            *pending = prompt_text.clone();
        }

        // Append the redo marker FIRST so the replay below resolves it as the final
        // pointer (updates.jsonl is append-only).
        self.persist_xai_update_only(XaiSessionUpdate::RewindMarker {
            target_prompt_index: timeline_target,
            created_at: chrono::Utc::now().to_rfc3339(),
            to_branch: Some(to_branch),
        });

        // Rebuild the conversation from the journal (replay resolves the branch chain
        // and any compaction checkpoints on it).
        let replay_updates = updates_path.clone();
        let replay_session_dir = session_dir.clone();
        let replay_target = timeline_target;
        let replay_result = tokio::task::spawn_blocking(move || {
            crate::session::helpers::replay::replay_to_prompt(
                &replay_updates,
                &replay_session_dir,
                replay_target,
            )
        })
        .await
        .map_err(|e| anyhow::anyhow!("spawn_blocking panicked: {e}"))?;
        let replay_result = match replay_result {
            Ok(r) => r,
            Err(e) => {
                tracing::error!(?e, to_branch, "Redo replay failed — rewind aborted");
                return Ok(error_response(
                    timeline_target,
                    format!("Cannot switch to branch #{to_branch}: {e}"),
                ));
            }
        };

        // Splice the rebuilt conversation, keeping the session preamble (same as the
        // cross-compaction path).
        let mut conversation = self.chat_state_handle.get_conversation().await;
        if matches!(replay_result.conversation.first(), Some(ConversationItem::System(_))) {
            conversation = replay_result.conversation;
        } else {
            if let Some(ui0) = replay_result.original_user_info {
                conversation.truncate(1); // keep System only
                conversation.push(ConversationItem::user(ui0));
            } else {
                conversation.truncate(2); // keep System + current user_info
            }
            conversation.extend(replay_result.conversation);
        }

        self.cancel_active_sampling_requests();
        self.cancel_pending_image_strips_for_rewind();
        self.chat_state_handle.replace_conversation(conversation);
        if let Some(mut snap) = self.chat_state_handle.snapshot().await {
            snap.prompt_index = timeline_target;
            // The switched-to branch's prompt texts are not in memory — re-derive them
            // from the journal (design §7-3: prompt_texts degrade to a derived cache).
            snap.prompt_texts = Self::load_user_prompts_from_updates(&updates_path)
                .unwrap_or_else(|_| snap.prompt_texts.clone());
            snap.last_compaction_prompt_index = replay_result.last_compaction_prompt_index;
            self.chat_state_handle.restore_snapshot(snap);
        }

        // Post-rewind housekeeping, identical to the forward path.
        if self
            .compaction
            .auto_compact_suppressed
            .load(std::sync::atomic::Ordering::Relaxed)
            != crate::session::compaction_config::SUPPRESS_UNTIL_SUCCESS
        {
            self.compaction.auto_compact_suppressed.store(
                crate::session::compaction_config::SUPPRESS_NONE,
                std::sync::atomic::Ordering::Relaxed,
            );
        }
        self.rearm_failed_server_announcements().await;
        self.recap_epoch.set(self.recap_epoch.get().wrapping_add(1));
        self.abort_turn_summary();
        self.abort_title_refresh();
        let _ = self
            .notifications
            .persistence_tx
            .send(PersistenceMsg::LastTurnSummary(None));
        let _ = self
            .notifications
            .persistence_tx
            .send(PersistenceMsg::LastRecap(None));
        if !crate::session::persistence::title_is_manual_in_dir(&session_dir) {
            let post_rewind_turns = crate::session::helpers::session_recap::main_turn_count(
                &self.chat_state_handle.get_conversation().await,
            );
            let idx = crate::session::helpers::session_summary::checkpoints_reached(
                post_rewind_turns,
            );
            self.next_title_refresh_idx.set(idx);
            crate::session::helpers::session_summary::save_title_refresh_watermark(
                &session_dir,
                idx,
            );
        }

        tracing::info!(
            to_branch,
            own_target = target_index,
            timeline_target,
            "redo rewind: switched back to abandoned branch"
        );

        Ok(RewindResponse {
            success: true,
            target_prompt_index: timeline_target,
            mode,
            reverted_files: vec![],
            clean_files: vec![],
            conflicts: vec![],
            prompt_text,
            error: None,
        })
    }

    /// `ConversationOnly` rewind-tracker bookkeeping: merge the discarded prompts' file effects (`>= target_index`) into the previous rewind point.
    /// The merge means a lazily-unloaded or partial tracker can't truncate history off disk.
    /// No normalize_to_relative needed: per-turn persistence already normalized the on-disk points (turn.rs, before PersistenceMsg::RewindPoint).
    pub(super) async fn merge_rewind_tracker_from(&self, target_index: usize) {
        self.file_state_tracker
            .merge_and_remove_from(target_index)
            .await;
        let _ = self
            .notifications
            .persistence_tx
            .send(PersistenceMsg::MergeRewindPointsFrom { target_index });
    }

    /// Out-of-band history repair (`x.ai/session/repair`) for a resident session.
    /// Runs `xai_chat_state::compaction_utils::repair_history` inside the chat-state actor, then flushes persistence.
    /// The flush means `chat_history.jsonl` is rewritten on disk before the caller sees success.
    pub(super) async fn handle_repair_history(
        &self,
        dry_run: bool,
    ) -> anyhow::Result<xai_chat_state::compaction_utils::HistoryRepairReport> {
        // Per-session flag, NOT `tool_context.is_turn_active`, which is the agent-wide coordinator flag shared by all sessions
        // Using it refuses repair of an idle session while any other session runs a turn, and another session's turn end could clear it mid-turn
        let turn_flag = self.session_turn_active.clone();
        if turn_flag.load(std::sync::atomic::Ordering::SeqCst) {
            anyhow::bail!(xai_chat_state::commands::RepairHistoryBlocked);
        }

        let report = self
            .chat_state_handle
            .repair_history(dry_run, Some(turn_flag))
            .await
            .ok_or_else(|| anyhow::anyhow!("chat-state actor unavailable"))?
            .map_err(anyhow::Error::new)?;

        if report.changed() && !dry_run {
            // Flush barrier: success must mean the rewrite is on disk.
            let (flush_tx, flush_rx) = tokio::sync::oneshot::channel();
            if self
                .notifications
                .persistence_tx
                .send(PersistenceMsg::FlushAndAck {
                    respond_to: flush_tx,
                })
                .is_err()
                || !matches!(flush_rx.await, Ok(Ok(())))
            {
                anyhow::bail!("history repaired in memory but the persistence flush failed");
            }
            tracing::warn!(
                session_id = %self.session_info.id.0,
                duplicates_removed = report.duplicates_removed,
                stripped_tool_result_ids = ?report.stripped_tool_result_ids,
                synthetic_results_inserted = report.synthetic_results_inserted,
                "session history repaired"
            );
        }

        Ok(report)
    }
}
