//! LOCAL (vision bridge): model-capability gating for image inputs.
//!
//! D1/D2/D3 from `docs-local/vision-bridge/plan.md`: a text-only main model
//! gets its images transcribed by a separate vision model (the bridge); a
//! vision-capable main model keeps today's native inline path. The bridge only
//! runs when authorized: the user pinned `[models] image_description`, or the
//! main model rides xAI session credentials. A BYOK text model with no pin
//! strips images and tells the user — never a silent xAI call.

use super::*;
use crate::extensions::notification::SessionUpdate as XaiSessionUpdate;

/// What to do with the images of the current turn / interjection / tool result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ImageBridgeDecision {
    /// Main model accepts images: keep today's native inline path.
    Native,
    /// Main model is text-only and the bridge is authorized: transcribe.
    Bridge,
    /// Main model is text-only and no bridge is available: strip and disclose.
    Blocked(VisionBridgeBlockedReason),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum VisionBridgeBlockedReason {
    /// D3: no explicit describe pin and the main model carries its own
    /// credentials (BYOK) — a default describe call would silently burn the
    /// xAI unified pool.
    Unauthorized,
    /// The configured describe model is absent from the catalog or itself
    /// cannot read images.
    NoVisionModel,
}

impl VisionBridgeBlockedReason {
    fn as_const_str(self) -> &'static str {
        match self {
            VisionBridgeBlockedReason::Unauthorized => "unauthorized_byok",
            VisionBridgeBlockedReason::NoVisionModel => "no_vision_model",
        }
    }
}

/// Result of the D4 failure-tolerant user-image transcription.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct UserImageTranscription {
    /// The final user-message text (describe envelope + `<image_files>` + original).
    pub text: String,
    /// Images successfully described.
    pub described: usize,
    /// Images whose describe call failed (stamped unavailable, omitted from the request).
    pub failed: usize,
    /// Images past the per-turn describe limit.
    pub skipped: usize,
    /// Describe model id (for the T3 disclosure).
    pub describe_model: String,
    /// Describe endpoint host, when parseable (cross-provider disclosure).
    pub describe_host: Option<String>,
}

/// Stamped in place of a description when the describe call failed (D4). The
/// upstream error text is logged, never embedded: it may carry URLs or tokens.
pub(crate) fn unavailable_marker(index: usize) -> String {
    format!("[Image {index} unavailable: transcription failed, omitted from this request]")
}

/// Host of an http(s) URL, for the cross-provider disclosure line.
pub(crate) fn xai_url_host(url: &str) -> Option<String> {
    url::Url::parse(url).ok()?.host_str().map(str::to_owned)
}

impl SessionActor {
    /// D2 + D3 gate, evaluated per image-bearing turn. Reads the CURRENT model
    /// (model switches re-resolve on the next turn) and the D3 authorization.
    pub(crate) async fn resolve_image_bridge(&self) -> ImageBridgeDecision {
        let explicit = crate::util::config::resolve_image_description_explicit();
        self.resolve_image_bridge_with(explicit).await
    }

    /// Testable core: the D3 "explicit describe pin" bit is injectable so tests
    /// never read the developer's real config.
    pub(crate) async fn resolve_image_bridge_with(
        &self,
        explicit_describe: bool,
    ) -> ImageBridgeDecision {
        let model_id = self.models_manager.current_model_id().0.to_string();
        if self
            .models_manager
            .model_accepts_images(&model_id)
            .unwrap_or(true)
        {
            return ImageBridgeDecision::Native;
        }
        let byok = self
            .models_manager
            .model_has_own_credentials(&model_id)
            .unwrap_or(false);
        if !explicit_describe && byok {
            let decision = ImageBridgeDecision::Blocked(VisionBridgeBlockedReason::Unauthorized);
            self.log_vision_bridge_decision(&model_id, &decision);
            return decision;
        }
        let describe_model = self.image_description_model.clone();
        let describe_capable = self
            .models_manager
            .model_accepts_images(&describe_model)
            .unwrap_or(false);
        let decision = if describe_capable {
            ImageBridgeDecision::Bridge
        } else {
            ImageBridgeDecision::Blocked(VisionBridgeBlockedReason::NoVisionModel)
        };
        self.log_vision_bridge_decision(&model_id, &decision);
        decision
    }

    fn log_vision_bridge_decision(&self, model_id: &str, decision: &ImageBridgeDecision) {
        let blocked = match decision {
            ImageBridgeDecision::Native => None,
            ImageBridgeDecision::Bridge => Some("bridge"),
            ImageBridgeDecision::Blocked(reason) => Some(reason.as_const_str()),
        };
        xai_grok_telemetry::unified_log::info(
            "shell.vision_bridge.decision",
            Some(self.session_info.id.0.as_ref()),
            Some(serde_json::json!({
                "model_id": model_id,
                "decision": blocked,
            })),
        );
    }

    /// T3 disclosure: one system scrollback line via the `ImageDropped` channel.
    pub(crate) async fn send_vision_bridge_notice(&self, message: String) {
        self.send_xai_notification(XaiSessionUpdate::VisionBridgeNotice { message })
            .await;
    }

    /// T2: describe one tool-result image under the D6 per-turn budget.
    /// `Ok(None)` = budget exhausted (caller stamps the skipped marker);
    /// `Err` = describe call failed (caller stamps the unavailable marker;
    /// upstream error text stays in the logs).
    pub(crate) async fn describe_tool_image(
        &self,
        tool_name: &str,
        call_id: &str,
        raw_bytes: &[u8],
        mime_type: &str,
        intent_text: &str,
    ) -> Result<Option<String>, crate::session::image_describe::DescribeError> {
        use crate::session::image_describe::{ImageDescribeSource, TOOL_IMAGE_INTENT_CAP};
        if self.image_describe_cache.take_tool_image_budget(1) == 0 {
            return Ok(None);
        }
        let active_session_config = self.reconstruct_full_config().await;
        let resolved_describe = self
            .resolve_aux_sampler_config(&self.image_description_model)
            .await;
        let (describe_model, sampler_config) =
            crate::agent::config::finalize_image_describe_sampler_config(
                resolved_describe,
                &active_session_config,
                self.client_identifier.clone(),
                Some(self.max_retries),
            );
        let client = xai_grok_sampler::SamplingClient::new(sampler_config)
            .map_err(|e| crate::session::image_describe::DescribeError::Sampling(format!("{e}")))?;
        let intent = xai_grok_tools::util::truncate::truncate_middle(
            &crate::session::image_describe::strip_template_context_tags(intent_text),
            TOOL_IMAGE_INTENT_CAP,
        );
        self.image_describe_cache
            .get_or_describe(
                client,
                &describe_model,
                raw_bytes,
                mime_type,
                None,
                &intent,
                ImageDescribeSource::ToolResult,
                &format!("{tool_name}:{call_id}"),
            )
            .await
            .map(Some)
    }
}

/// One user-visible disclosure line (T3). Shell-composed English, rendered by
/// the pager as a system scrollback block (same channel as `ImageDropped`).
pub(crate) fn render_vision_bridge_notice(
    described: usize,
    failed: usize,
    skipped: usize,
    describe_model: &str,
    describe_host: Option<&str>,
) -> String {
    let host = describe_host.unwrap_or("xAI");
    let mut parts = vec![format!(
        "Vision bridge: {described} image(s) transcribed with {describe_model} ({host}); \
         pixels were not sent to the main model."
    )];
    if failed > 0 {
        parts.push(format!("{failed} image(s) could not be described and were omitted."));
    }
    if skipped > 0 {
        parts.push(format!("{skipped} image(s) past the per-turn limit were skipped."));
    }
    parts.join(" ")
}

/// Disclosure for the blocked arms (D9 tier-1 / D3): nothing was sent.
pub(crate) fn render_vision_bridge_blocked_notice(reason: VisionBridgeBlockedReason) -> String {
    match reason {
        VisionBridgeBlockedReason::Unauthorized => {
            "Vision bridge off: this model cannot read images and no vision model is \
             configured (set `[models] image_description` in config.toml to allow one). \
             The image(s) were not sent."
                .to_string()
        }
        VisionBridgeBlockedReason::NoVisionModel => {
            "Vision bridge off: the configured vision model is unavailable or cannot read \
             images. The image(s) were not sent."
                .to_string()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::config::{EndpointsConfig, ModelEntry};
    use crate::test_support::TEST_MODEL;

    fn vision_entry(model: &str, accepts: Option<bool>, api_key: Option<&str>) -> ModelEntry {
        let mut entry =
            ModelEntry::fallback(model, &crate::agent::config::EndpointsConfig::default());
        entry.info.model = model.to_owned();
        entry.info.accepts_images = accepts;
        entry.api_key = api_key.map(str::to_owned);
        entry
    }

    async fn decision_for(
        main: ModelEntry,
        describe: Option<ModelEntry>,
        explicit: bool,
    ) -> ImageBridgeDecision {
        let actor = crate::session::acp_session::support::create_test_actor(
            0,
            200_000,
            85,
            tokio::sync::mpsc::unbounded_channel().0,
            tokio::sync::mpsc::unbounded_channel().0,
        )
        .await;
        let main_id = "main-model";
        actor.models_manager.insert_test_entry(main_id, main);
        actor.models_manager.set_current_model_id(acp::ModelId::new(
            std::sync::Arc::<str>::from(main_id),
        ));
        if let Some(describe) = describe {
            actor.models_manager.insert_test_entry(TEST_MODEL, describe);
        }
        actor.resolve_image_bridge_with(explicit).await
    }

    /// D1/D2: a vision-capable main model keeps the native path, whatever the
    /// describe config says.
    #[tokio::test(flavor = "current_thread")]
    async fn vision_capable_main_model_stays_native() {
        let main = vision_entry("grok-4.6", Some(true), None);
        let describe = vision_entry(TEST_MODEL, Some(false), None);
        let decision = decision_for(main, Some(describe), true).await;
        assert_eq!(decision, ImageBridgeDecision::Native);
    }

    /// D2: a text main model with an explicit pin and a vision describe model
    /// bridges.
    #[tokio::test(flavor = "current_thread")]
    async fn text_main_model_with_explicit_pin_bridges() {
        let main = vision_entry("text-model", Some(false), Some("sk-byok"));
        let describe = vision_entry(TEST_MODEL, Some(true), None);
        let decision = decision_for(main, Some(describe), true).await;
        assert_eq!(decision, ImageBridgeDecision::Bridge);
    }

    /// D3: BYOK main model without an explicit pin never bridges (no silent xAI
    /// spend) even though the describe model is available.
    #[tokio::test(flavor = "current_thread")]
    async fn byok_main_model_without_pin_is_blocked() {
        let main = vision_entry("text-model", Some(false), Some("sk-byok"));
        let describe = vision_entry(TEST_MODEL, Some(true), None);
        let decision = decision_for(main, Some(describe), false).await;
        assert_eq!(
            decision,
            ImageBridgeDecision::Blocked(VisionBridgeBlockedReason::Unauthorized)
        );
    }

    /// D3: an xAI-session main model (no own credentials) may use the default
    /// describe model.
    #[tokio::test(flavor = "current_thread")]
    async fn xai_session_main_model_without_pin_bridges() {
        let main = vision_entry("text-model", Some(false), None);
        let describe = vision_entry(TEST_MODEL, Some(true), None);
        let decision = decision_for(main, Some(describe), false).await;
        assert_eq!(decision, ImageBridgeDecision::Bridge);
    }

    /// D2: the describe model must itself be vision-capable; a text describe
    /// model (or an absent one) blocks instead of sampling into a guaranteed 400.
    #[tokio::test(flavor = "current_thread")]
    async fn text_only_describe_model_blocks() {
        let describe = vision_entry(TEST_MODEL, Some(false), None);
        let decision = decision_for(
            vision_entry("text-model", Some(false), None),
            Some(describe),
            true,
        )
        .await;
        assert_eq!(
            decision,
            ImageBridgeDecision::Blocked(VisionBridgeBlockedReason::NoVisionModel)
        );
        let decision = decision_for(
            vision_entry("text-model", Some(false), None),
            None,
            true,
        )
        .await;
        assert_eq!(
            decision,
            ImageBridgeDecision::Blocked(VisionBridgeBlockedReason::NoVisionModel)
        );
    }

    #[test]
    fn unavailable_marker_names_the_image_index() {
        assert_eq!(
            unavailable_marker(2),
            "[Image 2 unavailable: transcription failed, omitted from this request]"
        );
    }

    #[test]
    fn url_host_extracts_host_and_tolerates_garbage() {
        assert_eq!(
            xai_url_host("https://api.deepseek.com/v1").as_deref(),
            Some("api.deepseek.com")
        );
        assert_eq!(xai_url_host("not a url"), None);
    }

    #[test]
    fn bridge_notice_counts_each_bucket() {
        let notice = render_vision_bridge_notice(2, 1, 3, "my-vl", Some("api.example.com"));
        assert!(notice.contains("2 image(s) transcribed with my-vl (api.example.com)"));
        assert!(notice.contains("1 image(s) could not be described"));
        assert!(notice.contains("3 image(s) past the per-turn limit"));
        let notice = render_vision_bridge_notice(1, 0, 0, "grok-4.6", None);
        assert!(notice.contains("(xAI)"), "missing host falls back to xAI: {notice}");
        assert!(!notice.contains("could not be described"));
    }

    #[test]
    fn blocked_notices_name_the_reason() {
        let notice =
            render_vision_bridge_blocked_notice(VisionBridgeBlockedReason::Unauthorized);
        assert!(notice.contains("image_description"));
        let notice = render_vision_bridge_blocked_notice(VisionBridgeBlockedReason::NoVisionModel);
        assert!(notice.contains("unavailable or cannot read"));
    }

    /// D6: the tool-image budget grants at most four per turn and resets per turn.
    #[test]
    fn tool_image_budget_grants_then_resets() {
        let cache = crate::session::image_describe::ImageDescribeCache::new();
        assert_eq!(cache.take_tool_image_budget(3), 3);
        assert_eq!(cache.take_tool_image_budget(3), 1);
        assert_eq!(cache.take_tool_image_budget(1), 0);
        cache.reset_tool_image_budget();
        assert_eq!(cache.take_tool_image_budget(4), 4);
    }
}
