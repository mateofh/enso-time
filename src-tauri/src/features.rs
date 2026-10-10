use crate::models::SettingsV1;

/// OpenRouter evaluation and screen OCR ship in v0.3. Until then the code is
/// compiled and tested, but every entry point refuses to act unless the app is
/// built with `--features experimental-intelligence`.
pub const EXPERIMENTAL_INTELLIGENCE: bool = cfg!(feature = "experimental-intelligence");

pub fn require_experimental_intelligence() -> Result<(), String> {
    if EXPERIMENTAL_INTELLIGENCE {
        Ok(())
    } else {
        Err("La evaluación con IA y el OCR de pantalla no están disponibles en esta versión".into())
    }
}

/// Settings as this build is allowed to apply them: without the experimental
/// feature nothing may enable cloud evaluation or screen capture, even if a
/// settings payload or an older database asks for it.
pub fn effective_settings(settings: SettingsV1) -> SettingsV1 {
    if EXPERIMENTAL_INTELLIGENCE {
        return settings;
    }
    let mut deep_capture = settings.deep_capture.clone();
    deep_capture.screenshots.enabled = false;
    for source in [
        &mut deep_capture.browser,
        &mut deep_capture.editor,
        &mut deep_capture.shell,
        &mut deep_capture.screenshots,
    ] {
        source.cloud_allowed = false;
    }
    let mut ai = settings.ai.clone();
    ai.enabled = false;
    ai.weekly_enabled = false;
    SettingsV1 {
        deep_capture,
        ai,
        ..settings
    }
}

#[cfg(all(test, not(feature = "experimental-intelligence")))]
mod tests {
    use super::*;

    #[test]
    fn release_builds_strip_cloud_and_ocr_settings() {
        let mut settings = SettingsV1::default();
        settings.ai.enabled = true;
        settings.ai.weekly_enabled = true;
        settings.deep_capture.screenshots.enabled = true;
        settings.deep_capture.editor.enabled = true;
        settings.deep_capture.editor.cloud_allowed = true;
        settings.deep_capture.browser.cloud_allowed = true;

        let effective = effective_settings(settings);

        assert!(!effective.ai.enabled);
        assert!(!effective.ai.weekly_enabled);
        assert!(!effective.deep_capture.screenshots.enabled);
        assert!(
            effective.deep_capture.editor.enabled,
            "local connectors stay available"
        );
        assert!(!effective.deep_capture.editor.cloud_allowed);
        assert!(!effective.deep_capture.browser.cloud_allowed);
        assert!(require_experimental_intelligence().is_err());
    }
}
