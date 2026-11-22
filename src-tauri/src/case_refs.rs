//! Case identifiers (ticket, invoice, order numbers) found in window titles, kept only
//! as `type:hash`. They let process mapping follow one case across apps (the same
//! invoice in Gmail, Sheets and the CRM) without storing the identifier itself.
//!
//! Extraction runs at read time on titles the person chose to keep. Title redaction
//! already removes emails, card-like numbers and phones; these short identifiers
//! survive it, so nothing new is captured. The hash is an HMAC-SHA256 with a random
//! salt that never leaves the device and is deleted with the local data, so hashes are
//! not comparable across devices or after a reset.

use std::sync::LazyLock;

use regex::Regex;
use sha2::{Digest, Sha256};

use crate::models::TimelineBlockV1;

const SALT_KEY: &str = "case_ref_salt";
/// Hex characters of the HMAC kept per reference: enough to tell cases apart locally.
const HASH_HEX_CHARS: usize = 12;
const MAX_REFS_PER_BLOCK: usize = 4;

static PATTERNS: LazyLock<Vec<(&'static str, Regex)>> = LazyLock::new(|| {
    [
        ("ticket", r"#(\d{3,10})\b"),
        (
            "invoice",
            r"(?i)\b(?:INV|FAC|FV|FACT)[-_ ]?(\d{2,}(?:[-_]\d+)*)\b",
        ),
        ("order", r"(?i)\b(?:PO|OC)[-_ ]?(\d{3,}(?:[-_]\d+)*)\b"),
    ]
    .into_iter()
    .map(|(kind, pattern)| {
        (
            kind,
            Regex::new(pattern).expect("built-in case pattern is valid"),
        )
    })
    .collect()
});

fn hmac_sha256(key: &[u8], message: &[u8]) -> [u8; 32] {
    const BLOCK: usize = 64;
    let mut key_block = [0u8; BLOCK];
    if key.len() > BLOCK {
        key_block[..32].copy_from_slice(&Sha256::digest(key));
    } else {
        key_block[..key.len()].copy_from_slice(key);
    }
    let mut inner = Sha256::new();
    inner.update(key_block.map(|byte| byte ^ 0x36));
    inner.update(message);
    let mut outer = Sha256::new();
    outer.update(key_block.map(|byte| byte ^ 0x5c));
    outer.update(inner.finalize());
    outer.finalize().into()
}

/// `type:hash` references for the identifiers in a title, deduplicated, in order.
pub fn extract(title: &str, salt: &str) -> Vec<String> {
    let mut refs = Vec::new();
    for (kind, regex) in PATTERNS.iter() {
        for captures in regex.captures_iter(title) {
            let Some(value) = captures.get(1) else {
                continue;
            };
            let normalized = value.as_str().replace(['-', '_'], "");
            let digest = hmac_sha256(salt.as_bytes(), format!("{kind}:{normalized}").as_bytes());
            let hex = digest
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>();
            let reference = format!("{kind}:{}", &hex[..HASH_HEX_CHARS]);
            if !refs.contains(&reference) {
                refs.push(reference);
            }
        }
    }
    refs.truncate(MAX_REFS_PER_BLOCK);
    refs
}

/// The device's salt, created on first use and removed when local data is deleted.
pub fn local_salt(store: &crate::storage::LocalStore) -> Result<String, String> {
    if let Some(salt) = store.get_value(SALT_KEY)?.filter(|value| value.len() >= 32) {
        return Ok(salt);
    }
    let salt = format!(
        "{}{}",
        uuid::Uuid::new_v4().simple(),
        uuid::Uuid::new_v4().simple()
    );
    store.set_value(SALT_KEY, &salt)?;
    Ok(salt)
}

/// Copies of the blocks with `case_refs` filled from their (already redacted) titles.
pub fn annotate(blocks: &[TimelineBlockV1], salt: &str) -> Vec<TimelineBlockV1> {
    blocks
        .iter()
        .map(|block| TimelineBlockV1 {
            case_refs: block
                .title
                .as_deref()
                .map(|title| extract(title, salt))
                .unwrap_or_default(),
            ..block.clone()
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const SALT: &str = "0123456789abcdef0123456789abcdef";

    #[test]
    fn typed_references_hide_the_identifier_and_match_across_apps() {
        let gmail = extract("Re: Factura FAC-2024-0042 — Gmail", SALT);
        let sheets = extract("FAC_20240042.xlsx", SALT);
        assert_eq!(gmail.len(), 1);
        assert!(gmail[0].starts_with("invoice:"));
        assert_eq!(gmail[0].len(), "invoice:".len() + HASH_HEX_CHARS);
        // Separators are normalized, so the same invoice matches in both apps.
        assert_eq!(gmail, sheets);
        assert!(!gmail[0].contains("2024") && !gmail[0].contains("0042"));

        let ticket = extract("Ticket #48213 – Zendesk", SALT);
        assert!(ticket[0].starts_with("ticket:"));
        assert!(extract("PO-55120 approval", SALT)[0].starts_with("order:"));
        assert!(extract("Weekly sync — Google Meet", SALT).is_empty());
        assert!(
            extract("Issue #12", SALT).is_empty(),
            "too short to be a case id"
        );
    }

    #[test]
    fn salt_is_created_once_and_removed_with_local_data() {
        let directory = tempfile::TempDir::new().unwrap();
        let store =
            crate::storage::LocalStore::open(&directory.path().join("salt.sqlite")).unwrap();
        let first = local_salt(&store).unwrap();
        assert_eq!(first.len(), 64);
        assert_eq!(local_salt(&store).unwrap(), first);
        store.delete_all().unwrap();
        assert_ne!(local_salt(&store).unwrap(), first);
    }

    #[test]
    fn annotated_blocks_and_their_exports_never_carry_the_identifier() {
        let block = TimelineBlockV1 {
            id: "w1".into(),
            start: "2026-11-21T10:00:00Z".into(),
            end: "2026-11-21T10:10:00Z".into(),
            duration_seconds: 600,
            app: "Excel".into(),
            title: Some("FAC-2024-0042.xlsx".into()),
            activity_label: Some("Facturar".into()),
            category: "admin".into(),
            confidence: 1.0,
            afk: false,
            source: "window".into(),
            check_in_id: None,
            classification_source: "rule".into(),
            rule_id: None,
            correctable: true,
            context_domain: None,
            context_project: None,
            case_refs: Vec::new(),
        };
        let annotated = annotate(std::slice::from_ref(&block), SALT);
        assert_eq!(annotated[0].case_refs.len(), 1);
        let episodes = crate::episodes::build_episodes(&annotated);
        let ocel = crate::ocel::ocel_export(&episodes).to_string();
        let export =
            serde_json::to_string(&crate::models::ExportTimelineBlockV2::from(&annotated[0]))
                .unwrap();
        let evidence =
            serde_json::to_string(&crate::episodes::observed_evidence(&episodes)).unwrap();
        for output in [&ocel, &export, &evidence, &annotated[0].case_refs.join(",")] {
            assert!(!output.contains("0042"), "identifier leaked: {output}");
        }
        assert!(!ocel.contains("invoice:"), "case refs stay out of OCEL");
        assert!(
            !export.contains("invoice:"),
            "case refs stay out of exports"
        );
    }

    #[test]
    fn a_different_salt_gives_unrelated_hashes() {
        assert_ne!(
            extract("Ticket #48213", SALT),
            extract("Ticket #48213", "fedcba9876543210fedcba9876543210")
        );
    }

    #[test]
    fn hmac_matches_the_rfc_4231_test_vector() {
        // RFC 4231, test case 2.
        let digest = hmac_sha256(b"Jefe", b"what do ya want for nothing?");
        let hex = digest
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        assert_eq!(
            hex,
            "5bdcc146bf60754e6a042426089575c75a003f089d2739839dec58b964ec3843"
        );
    }
}
