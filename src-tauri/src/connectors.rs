use crate::commands::CaptureGate;
use crate::intelligence::{save_context_observation, ContextObservationV2};
use crate::models::{iso_now, parse_utc};
use crate::storage::LocalStore;
use rocket::data::{Data, ToByteUnit};
use rocket::http::Status;
use rocket::request::{FromRequest, Outcome};
use rocket::serde::json::Json;
use rocket::{post, routes, Request, Rocket, State};
use rusqlite::{params, OptionalExtension};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use uuid::Uuid;

pub const CHROMIUM_EXTENSION_ID: &str = "bbpphkeokmlgbpkkfmmpckpldffjabjg";
pub const CHROMIUM_EXTENSION_ORIGIN: &str = "chrome-extension://bbpphkeokmlgbpkkfmmpckpldffjabjg";
// `chrome-extension://` is an opaque origin to `rocket_cors`, so it cannot be
// registered as an exact origin. Keep the regex fully anchored to the packaged
// extension ID instead of widening access to arbitrary browser extensions.
pub const CHROMIUM_EXTENSION_ORIGIN_REGEX: &str =
    "^chrome-extension://bbpphkeokmlgbpkkfmmpckpldffjabjg$";

const PAIRING_CODE_LENGTH: usize = 8;
const PAIRING_CODE_SPACE: u32 = 100_000_000;
const PAIRING_TTL_MINUTES: i64 = 5;
// Any local process can reach the gateway, so a code must not be guessable by
// retrying: after this many failures every pending code is discarded.
const MAX_FAILED_PAIRING_ATTEMPTS: u32 = 5;
const PAIRING_LOCKOUT_SECONDS: i64 = 60;
// The embedded server accepts 1000 MB JSON bodies for bucket imports; the
// unauthenticated connector routes read with their own cap instead.
const MAX_CONNECTOR_BODY_KIB: u64 = 64;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectorPairingV1 {
    pub code: String,
    pub kind: String,
    pub name: String,
    pub origin: Option<String>,
    pub expires_at: String,
    pub server_port: Option<u16>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectorClientV1 {
    pub id: String,
    pub kind: String,
    pub name: String,
    pub origin: Option<String>,
    pub created_at: String,
    pub last_seen_at: Option<String>,
    pub revoked_at: Option<String>,
}

#[derive(Clone)]
struct PendingPairing {
    kind: String,
    name: String,
    origin: Option<String>,
    expires_at: String,
}

#[derive(Default)]
struct PairingThrottle {
    failures: u32,
    last_failure_at: Option<chrono::DateTime<chrono::Utc>>,
    locked_until: Option<chrono::DateTime<chrono::Utc>>,
}

pub struct ConnectorGateway {
    store: Arc<LocalStore>,
    capture_gate: Arc<CaptureGate>,
    pending: Mutex<HashMap<String, PendingPairing>>,
    throttle: Mutex<PairingThrottle>,
}

/// Errors carry the HTTP status the connector should see.
type GatewayError = (Status, String);

fn bad_request(error: String) -> GatewayError {
    (Status::BadRequest, error)
}

impl ConnectorGateway {
    pub fn new(store: Arc<LocalStore>, capture_gate: Arc<CaptureGate>) -> Self {
        Self {
            store,
            capture_gate,
            pending: Mutex::new(HashMap::new()),
            throttle: Mutex::new(PairingThrottle::default()),
        }
    }

    pub fn create_pairing(
        &self,
        kind: &str,
        name: &str,
        server_port: Option<u16>,
    ) -> Result<ConnectorPairingV1, String> {
        if !matches!(kind, "browser" | "editor" | "shell") {
            return Err("El tipo de conector no es válido".into());
        }
        let name = name.trim();
        if name.is_empty() || name.chars().count() > 100 || name.chars().any(char::is_control) {
            return Err("El nombre del conector no es válido".into());
        }
        let origin = (kind == "browser").then(|| CHROMIUM_EXTENSION_ORIGIN.to_string());
        let expires_at =
            (chrono::Utc::now() + chrono::Duration::minutes(PAIRING_TTL_MINUTES)).to_rfc3339();
        let mut pending = self
            .pending
            .lock()
            .map_err(|_| "No se pudo preparar el emparejamiento")?;
        pending.retain(|_, value| {
            parse_utc(&value.expires_at).is_ok_and(|expires| expires > chrono::Utc::now())
        });
        let code = (0..32)
            .find_map(|_| {
                let digest = Sha256::digest(Uuid::new_v4().as_bytes());
                let value = u32::from_be_bytes([digest[0], digest[1], digest[2], digest[3]])
                    % PAIRING_CODE_SPACE;
                let code = format!("{value:0width$}", width = PAIRING_CODE_LENGTH);
                (!pending.contains_key(&code)).then_some(code)
            })
            .ok_or("No se pudo generar un código de emparejamiento")?;
        pending.insert(
            code.clone(),
            PendingPairing {
                kind: kind.into(),
                name: name.into(),
                origin: origin.clone(),
                expires_at: expires_at.clone(),
            },
        );
        Ok(ConnectorPairingV1 {
            code,
            kind: kind.into(),
            name: name.into(),
            origin,
            expires_at,
            server_port,
        })
    }

    fn consume_pairing(
        &self,
        code: &str,
        request_origin: Option<&str>,
    ) -> Result<PendingPairing, String> {
        let mut pending = self
            .pending
            .lock()
            .map_err(|_| "No se pudo validar el emparejamiento")?;
        let pairing = pending
            .remove(code)
            .ok_or("El código de emparejamiento no existe o ya fue usado")?;
        if parse_utc(&pairing.expires_at)? <= chrono::Utc::now() {
            return Err("El código de emparejamiento venció".into());
        }
        if pairing
            .origin
            .as_deref()
            .is_some_and(|expected| request_origin != Some(expected))
        {
            return Err("El origen del conector no coincide".into());
        }
        Ok(pairing)
    }

    fn ensure_not_locked_out(&self) -> Result<(), GatewayError> {
        let mut throttle = self
            .throttle
            .lock()
            .map_err(|_| bad_request("No se pudo validar el emparejamiento".into()))?;
        match throttle.locked_until {
            Some(until) if until > chrono::Utc::now() => Err((
                Status::TooManyRequests,
                "Demasiados intentos de emparejamiento; genera un código nuevo en un minuto".into(),
            )),
            Some(_) => {
                *throttle = PairingThrottle::default();
                Ok(())
            }
            None => Ok(()),
        }
    }

    fn register_failed_pairing(&self) {
        let Ok(mut throttle) = self.throttle.lock() else {
            return;
        };
        let now = chrono::Utc::now();
        // Failures only accumulate within one code's lifetime, so sporadic typos
        // spread over days never add up to a lockout.
        let window = chrono::Duration::minutes(PAIRING_TTL_MINUTES);
        if throttle
            .last_failure_at
            .is_some_and(|last| now - last > window)
        {
            throttle.failures = 0;
        }
        throttle.failures += 1;
        throttle.last_failure_at = Some(now);
        if throttle.failures >= MAX_FAILED_PAIRING_ATTEMPTS {
            throttle.locked_until = Some(now + chrono::Duration::seconds(PAIRING_LOCKOUT_SECONDS));
            throttle.failures = 0;
            drop(throttle);
            self.clear_pending();
        }
    }

    fn pair(
        &self,
        request: PairRequest,
        request_origin: Option<&str>,
    ) -> Result<PairResponse, GatewayError> {
        self.ensure_not_locked_out()?;
        let operation = self.capture_gate.operation().map_err(bad_request)?;
        let well_formed = request.code.len() == PAIRING_CODE_LENGTH
            && request
                .code
                .chars()
                .all(|character| character.is_ascii_digit());
        let consumed = if well_formed {
            self.consume_pairing(&request.code, request_origin)
        } else {
            Err("El código de emparejamiento no es válido".into())
        };
        let pairing = consumed.map_err(|error| {
            self.register_failed_pairing();
            bad_request(error)
        })?;
        let response = self
            .complete_pairing(pairing)
            .and_then(|response| {
                operation.ensure_current()?;
                Ok(response)
            })
            .map_err(bad_request)?;
        if let Ok(mut throttle) = self.throttle.lock() {
            *throttle = PairingThrottle::default();
        }
        Ok(response)
    }

    fn complete_pairing(&self, pairing: PendingPairing) -> Result<PairResponse, String> {
        let settings = self.store.settings()?;
        let enabled = match pairing.kind.as_str() {
            "browser" => settings.deep_capture.browser.enabled,
            "editor" => settings.deep_capture.editor.enabled,
            "shell" => settings.deep_capture.shell.enabled,
            _ => false,
        };
        if !enabled {
            return Err("Esta fuente fue deshabilitada en Ensō Time".into());
        }
        // The id is always minted here: a client-chosen id could resurrect or
        // overwrite another connector's row.
        let client_id = Uuid::new_v4().to_string();
        let token = format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple());
        let token_hash = token_hash(&token);
        let now = iso_now();
        self.store
            .connection()?
            .execute(
                "INSERT INTO connector_clients
                 (id, kind, name, token_hash, origin, created_at, last_seen_at, revoked_at)
                 VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?6, NULL)",
                params![
                    client_id,
                    pairing.kind,
                    pairing.name,
                    token_hash,
                    pairing.origin,
                    now,
                ],
            )
            .map_err(|error| error.to_string())?;
        Ok(PairResponse {
            client_id,
            token,
            kind: pairing.kind,
        })
    }

    fn authenticate(&self, token: &str) -> Result<ConnectorIdentity, String> {
        let operation = self.capture_gate.operation()?;
        if token.len() != 64 || !token.chars().all(|character| character.is_ascii_hexdigit()) {
            return Err("Token inválido".into());
        }
        let hash = token_hash(token);
        let client: Option<(String, String)> = self
            .store
            .connection()?
            .query_row(
                "SELECT id, kind FROM connector_clients
                 WHERE token_hash = ?1 AND revoked_at IS NULL",
                [&hash],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()
            .map_err(|error| error.to_string())?;
        let (client_id, kind) = client.ok_or("Token inválido o revocado")?;
        self.store
            .connection()?
            .execute(
                "UPDATE connector_clients SET last_seen_at = ?2 WHERE id = ?1",
                params![client_id, iso_now()],
            )
            .map_err(|error| error.to_string())?;
        operation.ensure_current()?;
        Ok(ConnectorIdentity { client_id, kind })
    }

    pub fn list_clients(&self) -> Result<Vec<ConnectorClientV1>, String> {
        let connection = self.store.connection()?;
        let mut statement = connection
            .prepare(
                "SELECT id, kind, name, origin, created_at, last_seen_at, revoked_at
                 FROM connector_clients ORDER BY created_at DESC",
            )
            .map_err(|error| error.to_string())?;
        let rows = statement
            .query_map([], |row| {
                Ok(ConnectorClientV1 {
                    id: row.get(0)?,
                    kind: row.get(1)?,
                    name: row.get(2)?,
                    origin: row.get(3)?,
                    created_at: row.get(4)?,
                    last_seen_at: row.get(5)?,
                    revoked_at: row.get(6)?,
                })
            })
            .map_err(|error| error.to_string())?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|error| error.to_string())
    }

    pub fn revoke_client(&self, id: &str) -> Result<(), String> {
        let operation = self.capture_gate.operation()?;
        if id.is_empty() || id.chars().count() > 128 {
            return Err("El conector no es válido".into());
        }
        let changed = self
            .store
            .connection()?
            .execute(
                "UPDATE connector_clients SET revoked_at = ?2 WHERE id = ?1 AND revoked_at IS NULL",
                params![id, iso_now()],
            )
            .map_err(|error| error.to_string())?;
        if changed == 0 {
            return Err("El conector ya no existe o estaba revocado".into());
        }
        operation.ensure_current()?;
        Ok(())
    }

    pub fn clear_pending(&self) {
        if let Ok(mut pending) = self.pending.lock() {
            pending.clear();
        }
    }

    fn ingest(
        &self,
        identity: &ConnectorIdentity,
        observation: ContextObservationV2,
    ) -> Result<ContextObservationV2, GatewayError> {
        // The token was checked before the body arrived; a client revoked while
        // its body was in flight must not land that observation.
        if !self.is_active(&identity.client_id).map_err(bad_request)? {
            return Err((Status::Unauthorized, "Token inválido o revocado".into()));
        }
        let settings = self.store.settings().map_err(bad_request)?;
        // Pausing or leaving the work schedule stops deep context exactly like
        // it stops screen OCR; connectors must not keep recording meanwhile.
        if !crate::commands::is_running(&self.store).map_err(bad_request)?
            || !crate::commands::inside_work_schedule(&settings, chrono::Utc::now())
        {
            return Err((
                Status::Locked,
                "Ensō Time está en pausa o fuera del horario de trabajo".into(),
            ));
        }
        self.store_observation(identity, observation, &settings)
            .map_err(bad_request)
    }

    fn is_active(&self, client_id: &str) -> Result<bool, String> {
        self.store
            .connection()?
            .query_row(
                "SELECT 1 FROM connector_clients WHERE id = ?1 AND revoked_at IS NULL",
                [client_id],
                |_| Ok(()),
            )
            .optional()
            .map(|row| row.is_some())
            .map_err(|error| error.to_string())
    }

    fn store_observation(
        &self,
        identity: &ConnectorIdentity,
        observation: ContextObservationV2,
        settings: &crate::models::SettingsV1,
    ) -> Result<ContextObservationV2, String> {
        let operation = self.capture_gate.operation()?;
        let expected_source = match identity.kind.as_str() {
            "browser" => "browser",
            "editor" => "editor",
            "shell" => "shell",
            _ => return Err("El conector no puede enviar observaciones".into()),
        };
        if observation.source != expected_source {
            return Err("El conector intentó enviar una fuente no autorizada".into());
        }
        let enabled = match identity.kind.as_str() {
            "browser" => settings.deep_capture.browser.enabled,
            "editor" => settings.deep_capture.editor.enabled,
            "shell" => settings.deep_capture.shell.enabled,
            _ => false,
        };
        if !enabled {
            return Err("Esta fuente fue deshabilitada en Ensō Time".into());
        }
        let mut observation = observation.normalize_and_validate()?;
        if identity.kind == "browser" {
            let domain = observation.domain.as_deref().unwrap_or_default();
            if domain_matches_any(domain, &settings.deep_capture.excluded_sites) {
                return Err("Este sitio está excluido por la política local".into());
            }
            if observation.text_excerpt.is_some()
                && !domain_matches_any(domain, &settings.deep_capture.browser_text_allowlist)
            {
                // The desktop policy is authoritative even if an extension retained
                // a broader optional site permission from an earlier configuration.
                observation.text_excerpt = None;
            }
        }
        observation.client_id = Some(identity.client_id.clone());
        let observed_at = parse_utc(&observation.observed_at)?;
        if (chrono::Utc::now() - observed_at).num_hours().abs() > 24 {
            return Err("La hora de la observación está fuera del rango permitido".into());
        }
        let saved = save_context_observation(&self.store, observation)?;
        operation.ensure_current()?;
        Ok(saved)
    }
}

fn domain_matches_any(domain: &str, patterns: &[String]) -> bool {
    let domain = domain.trim().trim_end_matches('.').to_ascii_lowercase();
    patterns.iter().any(|pattern| {
        let pattern = pattern.trim().trim_end_matches('.').to_ascii_lowercase();
        !pattern.is_empty() && (domain == pattern || domain.ends_with(&format!(".{pattern}")))
    })
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PairRequest {
    code: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct PairResponse {
    client_id: String,
    token: String,
    kind: String,
}

#[derive(Debug)]
struct ConnectorIdentity {
    client_id: String,
    kind: String,
}

struct RequestOrigin(Option<String>);

#[rocket::async_trait]
impl<'r> FromRequest<'r> for RequestOrigin {
    type Error = ();

    async fn from_request(request: &'r Request<'_>) -> Outcome<Self, Self::Error> {
        Outcome::Success(Self(request.headers().get_one("Origin").map(str::to_owned)))
    }
}

struct ConnectorAuth(ConnectorIdentity);

#[rocket::async_trait]
impl<'r> FromRequest<'r> for ConnectorAuth {
    type Error = String;

    async fn from_request(request: &'r Request<'_>) -> Outcome<Self, Self::Error> {
        let Some(gateway) = request.rocket().state::<Arc<ConnectorGateway>>() else {
            return Outcome::Error((Status::ServiceUnavailable, "Gateway no disponible".into()));
        };
        let Some(token) = request
            .headers()
            .get_one("Authorization")
            .and_then(|value| value.strip_prefix("Bearer "))
        else {
            return Outcome::Error((Status::Unauthorized, "Falta el token".into()));
        };
        match gateway.authenticate(token) {
            Ok(identity) => Outcome::Success(Self(identity)),
            Err(error) => Outcome::Error((Status::Unauthorized, error)),
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ConnectorError {
    error: String,
}

type ConnectorResponse<T> = Result<Json<T>, (Status, Json<ConnectorError>)>;

fn error_response((status, error): GatewayError) -> (Status, Json<ConnectorError>) {
    (status, Json(ConnectorError { error }))
}

async fn read_capped_json<T: DeserializeOwned>(data: Data<'_>) -> Result<T, GatewayError> {
    let body = data
        .open(MAX_CONNECTOR_BODY_KIB.kibibytes())
        .into_string()
        .await
        .map_err(|_| bad_request("No se pudo leer la solicitud".into()))?;
    if !body.is_complete() {
        return Err((
            Status::PayloadTooLarge,
            "La solicitud supera el tamaño permitido".into(),
        ));
    }
    serde_json::from_str(&body.into_inner())
        .map_err(|_| bad_request("La solicitud no es JSON válido".into()))
}

#[post("/pair", format = "json", data = "<body>")]
async fn pair(
    gateway: &State<Arc<ConnectorGateway>>,
    origin: RequestOrigin,
    body: Data<'_>,
) -> ConnectorResponse<PairResponse> {
    let request = read_capped_json::<PairRequest>(body)
        .await
        .map_err(error_response)?;
    gateway
        .pair(request, origin.0.as_deref())
        .map(Json)
        .map_err(error_response)
}

#[post("/observations", format = "json", data = "<body>")]
async fn observations(
    gateway: &State<Arc<ConnectorGateway>>,
    auth: ConnectorAuth,
    body: Data<'_>,
) -> ConnectorResponse<ContextObservationV2> {
    let observation = read_capped_json::<ContextObservationV2>(body)
        .await
        .map_err(error_response)?;
    gateway
        .ingest(&auth.0, observation)
        .map(Json)
        .map_err(error_response)
}

pub fn mount_gateway(
    rocket: Rocket<rocket::Build>,
    gateway: Arc<ConnectorGateway>,
) -> Rocket<rocket::Build> {
    rocket
        .manage(gateway)
        .mount("/connectors/v1", routes![pair, observations])
}

fn token_hash(token: &str) -> String {
    Sha256::digest(token.as_bytes())
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn pairing_is_single_use_and_tokens_are_not_stored_in_plaintext() {
        let temp = tempdir().unwrap();
        let store = Arc::new(LocalStore::open(&temp.path().join("enso.sqlite")).unwrap());
        let mut settings = store.settings().unwrap();
        settings.deep_capture.editor.enabled = true;
        store.save_settings(&settings).unwrap();
        let gate = Arc::new(CaptureGate::new());
        let gateway = ConnectorGateway::new(Arc::clone(&store), gate);
        let pairing = gateway
            .create_pairing("editor", "VS Code", Some(5610))
            .unwrap();
        assert_eq!(pairing.code.len(), PAIRING_CODE_LENGTH);
        let response = gateway
            .pair(
                PairRequest {
                    code: pairing.code.clone(),
                },
                None,
            )
            .unwrap();
        assert_eq!(response.token.len(), 64);
        assert!(Uuid::parse_str(&response.client_id).is_ok());
        assert!(gateway
            .pair(PairRequest { code: pairing.code }, None)
            .is_err());
        let stored: String = store
            .connection()
            .unwrap()
            .query_row(
                "SELECT token_hash FROM connector_clients WHERE id = ?1",
                [&response.client_id],
                |row| row.get(0),
            )
            .unwrap();
        assert_ne!(stored, response.token);
        assert_eq!(stored, token_hash(&response.token));
    }

    fn browser_observation(domain: &str, excerpt: &str) -> ContextObservationV2 {
        let now = iso_now();
        ContextObservationV2 {
            schema_version: 2,
            id: Uuid::new_v4().to_string(),
            source: "browser".into(),
            observed_at: now.clone(),
            duration_seconds: 0,
            app: Some("Chromium".into()),
            title: Some("A tab".into()),
            domain: Some(domain.into()),
            project: None,
            repository: None,
            branch: None,
            file_type: None,
            text_excerpt: Some(excerpt.into()),
            ocr_terms: vec![],
            sensitivity: "normal".into(),
            client_id: None,
            created_at: now,
        }
    }

    #[test]
    fn desktop_policy_disables_sources_and_strips_unapproved_browser_text() {
        let temp = tempdir().unwrap();
        let store = Arc::new(LocalStore::open(&temp.path().join("enso.sqlite")).unwrap());
        let gate = Arc::new(CaptureGate::new());
        let gateway = ConnectorGateway::new(Arc::clone(&store), gate);
        let identity = ConnectorIdentity {
            client_id: "browser-1".into(),
            kind: "browser".into(),
        };
        let settings = store.settings().unwrap();
        assert!(gateway
            .store_observation(
                &identity,
                browser_observation("example.com", "private text"),
                &settings,
            )
            .is_err());

        let mut settings = store.settings().unwrap();
        settings.deep_capture.browser.enabled = true;
        settings.deep_capture.browser_text_allowlist = vec!["allowed.example".into()];
        store.save_settings(&settings).unwrap();
        let stripped = gateway
            .store_observation(
                &identity,
                browser_observation("example.com", "private text"),
                &settings,
            )
            .unwrap();
        assert!(stripped.text_excerpt.is_none());
        let allowed = gateway
            .store_observation(
                &identity,
                browser_observation("docs.allowed.example", "approved text"),
                &settings,
            )
            .unwrap();
        assert_eq!(allowed.text_excerpt.as_deref(), Some("approved text"));
    }

    fn editor_gateway() -> (tempfile::TempDir, Arc<LocalStore>, ConnectorGateway) {
        let temp = tempdir().unwrap();
        let store = Arc::new(LocalStore::open(&temp.path().join("enso.sqlite")).unwrap());
        let mut settings = store.settings().unwrap();
        settings.deep_capture.editor.enabled = true;
        settings.deep_capture.browser.enabled = true;
        store.save_settings(&settings).unwrap();
        let gateway = ConnectorGateway::new(Arc::clone(&store), Arc::new(CaptureGate::new()));
        (temp, store, gateway)
    }

    fn paired_editor(gateway: &ConnectorGateway) -> ConnectorIdentity {
        let pairing = gateway.create_pairing("editor", "VS Code", None).unwrap();
        let paired = gateway
            .pair(PairRequest { code: pairing.code }, None)
            .unwrap();
        gateway.authenticate(&paired.token).unwrap()
    }

    #[test]
    fn repeated_wrong_codes_discard_pending_pairings_and_lock_out() {
        let (_temp, _store, gateway) = editor_gateway();
        let pairing = gateway.create_pairing("editor", "VS Code", None).unwrap();
        let wrong = if pairing.code == "00000000" {
            "11111111"
        } else {
            "00000000"
        };
        for _ in 0..MAX_FAILED_PAIRING_ATTEMPTS {
            let (status, _) = gateway
                .pair(PairRequest { code: wrong.into() }, None)
                .unwrap_err();
            assert_eq!(status, Status::BadRequest);
        }
        let (status, _) = gateway
            .pair(
                PairRequest {
                    code: pairing.code.clone(),
                },
                None,
            )
            .unwrap_err();
        assert_eq!(status, Status::TooManyRequests);
        gateway.throttle.lock().unwrap().locked_until = Some(chrono::Utc::now());
        let (status, _) = gateway
            .pair(PairRequest { code: pairing.code }, None)
            .unwrap_err();
        assert_eq!(
            status,
            Status::BadRequest,
            "the correct code must not survive a lockout"
        );
    }

    #[test]
    fn malformed_codes_count_as_failed_attempts() {
        let (_temp, _store, gateway) = editor_gateway();
        for _ in 0..MAX_FAILED_PAIRING_ATTEMPTS {
            assert!(gateway
                .pair(PairRequest { code: "12".into() }, None)
                .is_err());
        }
        assert_eq!(
            gateway
                .pair(
                    PairRequest {
                        code: "12345678".into()
                    },
                    None
                )
                .unwrap_err()
                .0,
            Status::TooManyRequests
        );
    }

    #[test]
    fn connectors_cannot_record_while_tracking_is_not_running() {
        let (_temp, store, gateway) = editor_gateway();
        let identity = paired_editor(&gateway);
        // Onboarding not completed means tracking is not running.
        let (status, _) = gateway
            .ingest(&identity, browser_observation("example.com", "text"))
            .unwrap_err();
        assert_eq!(status, Status::Locked);

        let mut settings = store.settings().unwrap();
        settings.onboarding_completed = true;
        store.save_settings(&settings).unwrap();
        store
            .set_paused_until(Some(
                &(chrono::Utc::now() + chrono::Duration::hours(1)).to_rfc3339(),
            ))
            .unwrap();
        let (status, _) = gateway
            .ingest(&identity, browser_observation("example.com", "text"))
            .unwrap_err();
        assert_eq!(status, Status::Locked);
        let stored: i64 = store
            .connection()
            .unwrap()
            .query_row("SELECT COUNT(*) FROM context_observations", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(stored, 0);
    }

    #[test]
    fn a_client_revoked_mid_request_cannot_store_its_observation() {
        let (_temp, _store, gateway) = editor_gateway();
        let identity = paired_editor(&gateway);
        gateway.revoke_client(&identity.client_id).unwrap();
        let (status, _) = gateway
            .ingest(&identity, browser_observation("example.com", "text"))
            .unwrap_err();
        assert_eq!(status, Status::Unauthorized);
    }

    #[test]
    fn unauthenticated_routes_reject_oversized_bodies_before_parsing() {
        use rocket::http::ContentType;
        use rocket::local::blocking::Client;
        let (_temp, _store, gateway) = editor_gateway();
        let pairing = gateway.create_pairing("editor", "VS Code", None).unwrap();
        let client = Client::untracked(mount_gateway(rocket::build(), Arc::new(gateway))).unwrap();
        let padding = "x".repeat((MAX_CONNECTOR_BODY_KIB as usize + 1) * 1024);
        let oversized = client
            .post("/connectors/v1/pair")
            .header(ContentType::JSON)
            .body(format!(
                r#"{{"code":"{}","pad":"{padding}"}}"#,
                pairing.code
            ))
            .dispatch();
        assert_eq!(oversized.status(), Status::PayloadTooLarge);
        let paired = client
            .post("/connectors/v1/pair")
            .header(ContentType::JSON)
            .body(format!(r#"{{"code":"{}"}}"#, pairing.code))
            .dispatch();
        assert_eq!(paired.status(), Status::Ok);
    }

    #[test]
    fn a_connector_cannot_overwrite_another_connectors_observation() {
        let (_temp, store, gateway) = editor_gateway();
        let settings = store.settings().unwrap();
        let first = ConnectorIdentity {
            client_id: "browser-1".into(),
            kind: "browser".into(),
        };
        let second = ConnectorIdentity {
            client_id: "browser-2".into(),
            kind: "browser".into(),
        };
        let original = browser_observation("example.com", "text");
        let mut forged = original.clone();
        forged.title = Some("forged".into());
        gateway
            .store_observation(&first, original.clone(), &settings)
            .unwrap();
        assert!(gateway
            .store_observation(&second, forged.clone(), &settings)
            .is_err());
        let title: String = store
            .connection()
            .unwrap()
            .query_row(
                "SELECT title FROM context_observations WHERE id = ?1",
                [&original.id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(title, "A tab");
        assert!(gateway.store_observation(&first, forged, &settings).is_ok());
    }

    #[test]
    fn stale_or_cleared_pairing_failures_do_not_accumulate() {
        let (_temp, _store, gateway) = editor_gateway();
        for _ in 0..MAX_FAILED_PAIRING_ATTEMPTS - 1 {
            assert!(gateway
                .pair(PairRequest { code: "12".into() }, None)
                .is_err());
        }
        gateway.throttle.lock().unwrap().last_failure_at =
            Some(chrono::Utc::now() - chrono::Duration::minutes(PAIRING_TTL_MINUTES + 1));
        assert_eq!(
            gateway
                .pair(PairRequest { code: "12".into() }, None)
                .unwrap_err()
                .0,
            Status::BadRequest
        );
        assert_eq!(gateway.throttle.lock().unwrap().failures, 1);

        let pairing = gateway.create_pairing("editor", "VS Code", None).unwrap();
        gateway
            .pair(PairRequest { code: pairing.code }, None)
            .unwrap();
        assert_eq!(gateway.throttle.lock().unwrap().failures, 0);
    }

    #[test]
    fn retention_timestamp_is_stamped_by_the_app() {
        let (_temp, store, gateway) = editor_gateway();
        let settings = store.settings().unwrap();
        let identity = ConnectorIdentity {
            client_id: "browser-1".into(),
            kind: "browser".into(),
        };
        let mut observation = browser_observation("example.com", "text");
        observation.created_at = "2999-01-01T00:00:00Z".into();
        let saved = gateway
            .store_observation(&identity, observation, &settings)
            .unwrap();
        assert!(saved.created_at.as_str() < "2999");
    }
}
