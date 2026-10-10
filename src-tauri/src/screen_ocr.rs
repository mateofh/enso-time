use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScreenOcrStatusV1 {
    pub supported: bool,
    pub permission: String,
    pub last_observation_at: Option<String>,
    pub last_error: Option<String>,
}

#[cfg(all(target_os = "macos", not(test)))]
mod platform {
    use objc2::rc::autoreleasepool;
    use objc2::runtime::AnyObject;
    use objc2::{AnyThread, ClassType};
    #[allow(deprecated)]
    use objc2_core_graphics::{
        CGDisplayCreateImage, CGMainDisplayID, CGPreflightScreenCaptureAccess,
        CGRequestScreenCaptureAccess,
    };
    use objc2_foundation::{NSArray, NSDictionary};
    use objc2_vision::{
        VNImageOption, VNImageRequestHandler, VNRecognizeTextRequest, VNRecognizedTextObservation,
        VNRequest, VNRequestTextRecognitionLevel,
    };
    use std::collections::BTreeSet;

    pub fn supported() -> bool {
        true
    }

    pub fn permission() -> &'static str {
        if CGPreflightScreenCaptureAccess() {
            "granted"
        } else {
            "required"
        }
    }

    pub fn request_permission() -> bool {
        CGRequestScreenCaptureAccess()
    }

    pub fn recognize_front_screen(
        excluded_apps: &[String],
    ) -> Result<(String, Vec<String>), String> {
        let active = crate::macos_capture::current_window(false).app;
        if excluded_apps
            .iter()
            .any(|excluded| excluded.eq_ignore_ascii_case(&active))
        {
            return Err("excluded-app".into());
        }
        if !CGPreflightScreenCaptureAccess() {
            return Err("screen-recording-permission-required".into());
        }

        autoreleasepool(|_| {
            #[allow(deprecated)]
            let image = CGDisplayCreateImage(CGMainDisplayID())
                .ok_or_else(|| "No se pudo capturar la pantalla principal".to_string())?;
            // Fast recognition and background processing keep the periodic task bounded.
            // `image` remains in memory and is released as soon as Vision returns.
            let request = VNRecognizeTextRequest::new();
            request.setRecognitionLevel(VNRequestTextRecognitionLevel::Fast);
            request.setUsesLanguageCorrection(false);
            request.setAutomaticallyDetectsLanguage(true);
            let base_request: &VNRequest = request.as_super().as_super();
            unsafe { base_request.setPreferBackgroundProcessing(true) };

            let requests = NSArray::from_slice(&[base_request]);
            let options: objc2::rc::Retained<NSDictionary<VNImageOption, AnyObject>> =
                NSDictionary::new();
            let handler = unsafe {
                VNImageRequestHandler::initWithCGImage_options(
                    VNImageRequestHandler::alloc(),
                    &image,
                    &options,
                )
            };
            handler
                .performRequests_error(&requests)
                .map_err(|error| format!("Vision OCR: {error}"))?;

            let mut terms = BTreeSet::new();
            if let Some(results) = unsafe { base_request.results() } {
                for observation in results.iter() {
                    let Some(text_observation) =
                        observation.downcast_ref::<VNRecognizedTextObservation>()
                    else {
                        continue;
                    };
                    let candidates = text_observation.topCandidates(1);
                    let Some(candidate) = candidates.firstObject() else {
                        continue;
                    };
                    collect_terms(&candidate.string().to_string(), &mut terms);
                    if terms.len() >= 30 {
                        break;
                    }
                }
            }
            Ok((active, terms.into_iter().take(30).collect()))
        })
    }

    fn collect_terms(value: &str, terms: &mut BTreeSet<String>) {
        for token in value
            .split(|character: char| {
                !character.is_alphanumeric() && character != '-' && character != '_'
            })
            .map(str::trim)
            .filter(|token| (3..=40).contains(&token.chars().count()))
        {
            if token.chars().any(char::is_numeric) && token.chars().count() > 12 {
                continue;
            }
            terms.insert(token.to_lowercase());
            if terms.len() >= 30 {
                return;
            }
        }
    }
}

#[cfg(not(all(target_os = "macos", not(test))))]
mod platform {
    pub fn supported() -> bool {
        false
    }
    pub fn permission() -> &'static str {
        "unsupported"
    }
    pub fn request_permission() -> bool {
        false
    }
    pub fn recognize_front_screen(
        _excluded_apps: &[String],
    ) -> Result<(String, Vec<String>), String> {
        Err("El OCR de pantalla todavía no está disponible en este sistema".into())
    }
}

pub use platform::{permission, recognize_front_screen, request_permission, supported};
