//! OCEL 2.0 JSON export of work episodes, for process-mining tools (PM4Py, Celonis,
//! ProM). Spec: <https://www.ocel-standard.org/> (arXiv 2403.01975).
//!
//! Events are episodes. Objects are the task (confirmed label), each tool or site
//! involved, and the local day. No window titles, project names or excluded apps are
//! included: steps are app identities or domains, as in the diagnostic.

use std::collections::BTreeMap;

use chrono::Local;
use serde_json::{json, Value};

use crate::episodes::Episode;

const UNLABELED_EVENT_TYPE: &str = "unlabeled";

fn event_type(episode: &Episode) -> String {
    episode
        .candidate_key
        .clone()
        .unwrap_or_else(|| UNLABELED_EVENT_TYPE.into())
}

pub fn ocel_export(episodes: &[Episode]) -> Value {
    let mut objects: BTreeMap<String, &'static str> = BTreeMap::new();
    let mut event_types: Vec<String> = Vec::new();
    let mut events = Vec::with_capacity(episodes.len());
    for (index, episode) in episodes.iter().enumerate() {
        let kind = event_type(episode);
        if !event_types.contains(&kind) {
            event_types.push(kind.clone());
        }
        let mut relationships = Vec::new();
        if let Some(task) = &episode.candidate_key {
            let id = format!("task:{task}");
            objects.insert(id.clone(), "task");
            relationships.push(json!({ "objectId": id, "qualifier": "belongs-to" }));
        }
        let mut seen = Vec::new();
        for step in &episode.steps {
            if seen.contains(&step) {
                continue;
            }
            seen.push(step);
            let id = format!("tool:{step}");
            objects.insert(id.clone(), "tool");
            relationships.push(json!({ "objectId": id, "qualifier": "performed-in" }));
        }
        let day = format!("day:{}", episode.start.with_timezone(&Local).date_naive());
        objects.insert(day.clone(), "day");
        relationships.push(json!({ "objectId": day, "qualifier": "belongs-to" }));
        events.push(json!({
            "id": format!("episode-{}", index + 1),
            "type": kind,
            "time": episode.start.to_rfc3339(),
            "attributes": [
                { "name": "touchSeconds", "value": episode.touch_seconds },
                { "name": "durationSeconds", "value": (episode.end - episode.start).num_seconds() },
                { "name": "steps", "value": episode.steps.join(" → ") },
            ],
            "relationships": relationships,
        }));
    }
    event_types.sort();
    let object_types = ["task", "tool", "day"]
        .iter()
        .map(|name| json!({ "name": name, "attributes": [] }))
        .collect::<Vec<_>>();
    let event_types = event_types
        .iter()
        .map(|name| {
            json!({
                "name": name,
                "attributes": [
                    { "name": "touchSeconds", "type": "integer" },
                    { "name": "durationSeconds", "type": "integer" },
                    { "name": "steps", "type": "string" },
                ],
            })
        })
        .collect::<Vec<_>>();
    let objects = objects
        .into_iter()
        .map(|(id, kind)| json!({ "id": id, "type": kind, "attributes": [], "relationships": [] }))
        .collect::<Vec<_>>();
    json!({
        "objectTypes": object_types,
        "eventTypes": event_types,
        "objects": objects,
        "events": events,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::TimelineBlockV1;
    use std::collections::HashSet;

    fn block(id: &str, from: &str, to: &str, app: &str, label: Option<&str>) -> TimelineBlockV1 {
        TimelineBlockV1 {
            id: id.into(),
            start: from.into(),
            end: to.into(),
            duration_seconds: (crate::models::parse_utc(to).unwrap()
                - crate::models::parse_utc(from).unwrap())
            .num_seconds(),
            app: app.into(),
            title: Some("Factura de ana@cliente.com — CONFIDENCIAL".into()),
            activity_label: label.map(Into::into),
            category: if label.is_some() { "admin" } else { "unknown" }.into(),
            confidence: 1.0,
            afk: false,
            source: "window".into(),
            check_in_id: None,
            classification_source: if label.is_some() { "rule" } else { "unknown" }.into(),
            rule_id: None,
            correctable: true,
            context_domain: (app == "Google Chrome").then(|| "mail.google.com".into()),
            context_project: Some("proyecto-secreto".into()),
        }
    }

    #[test]
    fn export_has_valid_ocel_structure_and_references_without_titles() {
        let blocks = vec![
            block(
                "a",
                "2026-11-21T10:00:00Z",
                "2026-11-21T10:05:00Z",
                "Google Chrome",
                Some("Facturar"),
            ),
            block(
                "b",
                "2026-11-21T10:05:00Z",
                "2026-11-21T10:15:00Z",
                "Excel",
                Some("Facturar"),
            ),
            block(
                "c",
                "2026-11-21T14:00:00Z",
                "2026-11-21T14:20:00Z",
                "Figma",
                None,
            ),
        ];
        let episodes = crate::episodes::build_episodes(&blocks);
        let ocel = ocel_export(&episodes);
        for key in ["objectTypes", "eventTypes", "objects", "events"] {
            assert!(ocel[key].is_array(), "{key}");
        }
        let object_types = ocel["objectTypes"]
            .as_array()
            .unwrap()
            .iter()
            .map(|item| item["name"].as_str().unwrap())
            .collect::<HashSet<_>>();
        let event_types = ocel["eventTypes"]
            .as_array()
            .unwrap()
            .iter()
            .map(|item| item["name"].as_str().unwrap())
            .collect::<HashSet<_>>();
        let objects = ocel["objects"].as_array().unwrap();
        let object_ids = objects
            .iter()
            .map(|item| {
                assert!(object_types.contains(item["type"].as_str().unwrap()));
                item["id"].as_str().unwrap()
            })
            .collect::<HashSet<_>>();
        let events = ocel["events"].as_array().unwrap();
        assert_eq!(events.len(), 2);
        for event in events {
            assert!(event_types.contains(event["type"].as_str().unwrap()));
            assert!(crate::models::parse_utc(event["time"].as_str().unwrap()).is_ok());
            for relationship in event["relationships"].as_array().unwrap() {
                assert!(object_ids.contains(relationship["objectId"].as_str().unwrap()));
            }
        }
        assert_eq!(events[0]["type"], "facturar");
        assert!(object_ids.contains("task:facturar"));
        assert!(object_ids.contains("tool:mail.google.com"));
        assert!(object_ids.contains("tool:excel"));
        assert_eq!(events[1]["type"], UNLABELED_EVENT_TYPE);

        let text = ocel.to_string();
        for private in ["ana@cliente.com", "CONFIDENCIAL", "proyecto-secreto"] {
            assert!(!text.contains(private), "{private} leaked");
        }
    }
}
