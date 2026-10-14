pub struct NativeCopy {
    pub open: &'static str,
    pub pause: &'static str,
    pub resume: &'static str,
    pub quit: &'static str,
    pub missing_title: &'static str,
    pub missing_body: &'static str,
    pub action_failed: &'static str,
}

pub fn native_copy(locale: &str) -> NativeCopy {
    if locale == "en" {
        NativeCopy {
            open: "Open Ensō Time",
            pause: "Pause for 30 minutes",
            resume: "Resume",
            quit: "Quit",
            missing_title: "Ensō Time · Missing context",
            missing_body: "What were you doing during the last interval?",
            action_failed: "Could not change tracking. Open Ensō Time to check its current status.",
        }
    } else {
        NativeCopy {
            open: "Abrir Ensō Time",
            pause: "Pausar 30 minutos",
            resume: "Reanudar",
            quit: "Salir",
            missing_title: "Ensō Time · Falta contexto",
            missing_body: "¿Qué estabas haciendo en el último intervalo?",
            action_failed:
                "No pudimos cambiar el seguimiento. Abre Ensō Time para revisar el estado actual.",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn tray_and_notifications_follow_the_selected_language() {
        let english = native_copy("en");
        assert_eq!(english.resume, "Resume");
        assert!(english.missing_body.starts_with("What"));
        assert_eq!(native_copy("es-419").resume, "Reanudar");
    }
}
