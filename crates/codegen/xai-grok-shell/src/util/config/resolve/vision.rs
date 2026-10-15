//! LOCAL (vision bridge): D3 authorization resolver.
//!
//! The vision bridge sends the user's images to a *describe* model. When that
//! model is the compiled default (`grok-4.6` on the xAI route) the describe
//! call burns the xAI unified pool — a silent cost a BYOK user never asked
//! for. So the bridge is authorized only when the user (or the environment)
//! explicitly pinned a describe model, or the session itself runs on xAI
//! credentials (Cursor-pipeline parity).

use toml::Value as TomlValue;

pub(crate) const ENV_IMAGE_DESCRIPTION_MODEL: &str = "GROK_IMAGE_DESCRIPTION_MODEL";

/// `[models] image_description` present and non-empty in the raw TOML layer.
pub(crate) fn image_description_explicit_from_toml(v: Option<&TomlValue>) -> Option<bool> {
    let value = v?.get("models")?.get("image_description")?.as_str()?;
    Some(!value.trim().is_empty())
}

/// Whether the describe model was explicitly pinned by the user
/// (`[models] image_description` or `GROK_IMAGE_DESCRIPTION_MODEL`).
/// Remote-settings defaults stay non-explicit: like the compiled default they
/// carry no local intent to spend a cross-provider describe call.
pub(crate) fn resolve_image_description_explicit() -> bool {
    if let Ok(v) = std::env::var(ENV_IMAGE_DESCRIPTION_MODEL)
        && !v.trim().is_empty()
    {
        return true;
    }
    crate::config::load_effective_config()
        .ok()
        .as_ref()
        .and_then(|v| image_description_explicit_from_toml(Some(v)))
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn toml_with_image_description(value: &str) -> TomlValue {
        toml::from_str(&format!("[models]\nimage_description = {value}")).unwrap()
    }

    #[test]
    fn explicit_when_non_empty() {
        assert_eq!(
            image_description_explicit_from_toml(Some(&toml_with_image_description(
                "\"my-vl\""
            ))),
            Some(true)
        );
    }

    #[test]
    fn whitespace_only_counts_as_unset() {
        assert_eq!(
            image_description_explicit_from_toml(Some(&toml_with_image_description(
                "\"   \""
            ))),
            Some(false)
        );
    }

    #[test]
    fn absent_or_empty_string_is_not_explicit() {
        let empty: TomlValue = toml::from_str("[models]\nimage_description = \"\"\n").unwrap();
        assert_eq!(
            image_description_explicit_from_toml(Some(&empty)),
            Some(false)
        );
        let no_key: TomlValue = toml::from_str("[models]\nweb_search = \"x\"\n").unwrap();
        assert_eq!(image_description_explicit_from_toml(Some(&no_key)), None);
        assert_eq!(image_description_explicit_from_toml(None), None);
    }
}
