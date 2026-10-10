use core_foundation::base::TCFType;
use core_foundation::boolean::CFBoolean;
use core_foundation::dictionary::CFDictionary;
use core_foundation::string::CFString;
use core_foundation_sys::base::{CFRelease, CFTypeRef};
use core_foundation_sys::dictionary::CFDictionaryRef;
use core_foundation_sys::string::CFStringRef;
use objc2::rc::autoreleasepool;
use objc2_app_kit::NSWorkspace;
use std::ffi::c_void;
use std::ptr;

type AXUIElementRef = *const c_void;
type AXError = i32;
const AX_ERROR_SUCCESS: AXError = 0;

#[link(name = "ApplicationServices", kind = "framework")]
unsafe extern "C" {
    fn AXIsProcessTrusted() -> u8;
    fn AXIsProcessTrustedWithOptions(options: CFDictionaryRef) -> u8;
    fn AXUIElementCreateApplication(pid: i32) -> AXUIElementRef;
    fn AXUIElementCopyAttributeValue(
        element: AXUIElementRef,
        attribute: CFStringRef,
        value: *mut CFTypeRef,
    ) -> AXError;
    fn CGEventSourceSecondsSinceLastEventType(state_id: i32, event_type: u32) -> f64;
}

const HID_SYSTEM_STATE: i32 = 1;
const ANY_INPUT_EVENT_TYPE: u32 = u32::MAX;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowObservation {
    pub app: String,
    pub title: Option<String>,
    pub accessibility_trusted: bool,
}

pub fn accessibility_is_trusted() -> bool {
    // SAFETY: This process-global Accessibility query has no preconditions.
    unsafe { AXIsProcessTrusted() != 0 }
}

pub fn request_accessibility_permission() -> bool {
    let options: CFDictionary<CFString, CFBoolean> = CFDictionary::from_CFType_pairs(&[(
        CFString::new("AXTrustedCheckOptionPrompt"),
        CFBoolean::true_value(),
    )]);
    // SAFETY: The dictionary is retained for the duration of the synchronous call.
    unsafe { AXIsProcessTrustedWithOptions(options.as_concrete_TypeRef()) != 0 }
}

pub fn idle_seconds() -> f64 {
    // SAFETY: Quartz accepts the process-global HID state and the documented
    // kCGAnyInputEventType sentinel without additional ownership requirements.
    let seconds =
        unsafe { CGEventSourceSecondsSinceLastEventType(HID_SYSTEM_STATE, ANY_INPUT_EVENT_TYPE) };
    if seconds.is_finite() && seconds >= 0.0 {
        seconds
    } else {
        0.0
    }
}

pub fn current_window(include_title: bool) -> WindowObservation {
    autoreleasepool(|_| {
        let Some(application) = NSWorkspace::sharedWorkspace().frontmostApplication() else {
            return WindowObservation {
                app: "Unknown application".into(),
                title: None,
                accessibility_trusted: !include_title || accessibility_is_trusted(),
            };
        };
        let app = application
            .localizedName()
            .or_else(|| application.bundleIdentifier())
            .map(|value| value.to_string())
            .unwrap_or_else(|| "Unknown application".into());
        if !include_title {
            return WindowObservation {
                app,
                title: None,
                accessibility_trusted: true,
            };
        }
        let trusted = accessibility_is_trusted();
        let title = trusted
            .then(|| copy_focused_window_title(application.processIdentifier()))
            .flatten()
            .filter(|value| !value.trim().is_empty());
        WindowObservation {
            app,
            title,
            accessibility_trusted: trusted,
        }
    })
}

fn copy_focused_window_title(pid: i32) -> Option<String> {
    // SAFETY: Every object returned by a Create/Copy function is released exactly once.
    unsafe {
        let focused_window_attribute = CFString::new("AXFocusedWindow");
        let title_attribute = CFString::new("AXTitle");
        let application = AXUIElementCreateApplication(pid);
        if application.is_null() {
            return None;
        }
        let mut window: CFTypeRef = ptr::null();
        let window_result = AXUIElementCopyAttributeValue(
            application,
            focused_window_attribute.as_concrete_TypeRef(),
            &mut window,
        );
        CFRelease(application.cast());
        if window_result != AX_ERROR_SUCCESS || window.is_null() {
            return None;
        }
        let mut title: CFTypeRef = ptr::null();
        let title_result = AXUIElementCopyAttributeValue(
            window.cast(),
            title_attribute.as_concrete_TypeRef(),
            &mut title,
        );
        CFRelease(window);
        if title_result != AX_ERROR_SUCCESS || title.is_null() {
            return None;
        }
        Some(CFString::wrap_under_create_rule(title.cast()).to_string())
    }
}
