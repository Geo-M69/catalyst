use crate::FeatureResponse;
use chrono::Utc;
use serde_json::Value;

pub fn build_canonical_features(
    maybe_data: Option<&Value>,
    has_achievements: Option<bool>,
    achievements_count: Option<i64>,
    has_cloud_saves: Option<bool>,
    cloud_details: Option<String>,
    controller_support: Option<String>,
) -> Vec<FeatureResponse> {
    let mut features: Vec<FeatureResponse> = Vec::new();
    let mut seen_keys: std::collections::HashSet<String> = std::collections::HashSet::new();

    let mut controller_level: Option<String> = None;

    if let Some(data) = maybe_data {
        if let Some(categories) = data.get("categories").and_then(Value::as_array) {
            let canonical_from_desc = |desc: &str| -> Option<(String, String)> {
                let lowered = desc.to_ascii_lowercase();
                if lowered.contains("remote play together") || lowered.contains("remote play") {
                    return Some(("family-sharing".to_string(), "Family Sharing".to_string()));
                }
                if lowered.contains("steam cloud") || lowered.contains("steam cloud saves") || lowered.contains("cloud saves") || lowered == "cloud" {
                    return Some(("cloud-saves".to_string(), "Cloud Saves".to_string()));
                }
                if lowered.contains("trading card") || lowered.contains("trading cards") || lowered.contains("trading-cards") {
                    return Some(("__skip__".to_string(), "".to_string()));
                }
                if lowered.contains("multi-player") || lowered.contains("multiplayer") {
                    return Some(("multi-player".to_string(), "Multi-Player".to_string()));
                }
                if lowered.contains("co-op") || lowered.contains("cooperative") {
                    return Some(("multi-player".to_string(), "Multi-Player".to_string()));
                }
                if lowered.contains("single-player") || lowered.contains("single player") {
                    return Some(("single-player".to_string(), "Single-Player".to_string()));
                }
                if lowered.contains("achievements") || lowered.contains("steam achievements") {
                    return Some(("achievements".to_string(), "Achievements".to_string()));
                }
                if lowered.contains("full controller") {
                    return Some(("controller-full".to_string(), "Full Controller Support".to_string()));
                }
                if lowered.contains("partial controller") {
                    return Some(("controller-partial".to_string(), "Partial Controller Support".to_string()));
                }
                if lowered.contains("workshop") {
                    return Some(("workshop".to_string(), "Steam Workshop".to_string()));
                }
                if lowered.contains("family sharing") || lowered.contains("family-share") || lowered.contains("family_share") {
                    return Some(("family-sharing".to_string(), "Family Sharing".to_string()));
                }
                None
            };

            for cat in categories {
                let id_opt = cat.get("id").and_then(Value::as_u64);
                let desc_opt = cat.get("description").and_then(Value::as_str).map(|s| s.to_string());
                if let Some(desc) = desc_opt.as_deref() {
                    if let Some((key, label)) = canonical_from_desc(desc) {
                        if key == "__skip__" {
                            continue;
                        }
                        if key == "controller-full" {
                            controller_level = Some("Full".to_string());
                            seen_keys.insert("controller-support".to_string());
                            continue;
                        }
                        if key == "controller-partial" {
                            if controller_level.is_none() {
                                controller_level = Some("Partial".to_string());
                                seen_keys.insert("controller-support".to_string());
                            }
                            continue;
                        }

                        if seen_keys.insert(key.clone()) {
                            features.push(FeatureResponse { key: key.clone(), label: label.clone(), icon: None, tooltip: None });
                        }
                        continue;
                    }
                }
                let label = desc_opt.clone().or_else(|| id_opt.map(|id| format!("Category {}", id))).unwrap_or_else(|| "Category".to_string());
                let key = if let Some(id) = id_opt { format!("category-{}", id) } else { label.to_ascii_lowercase().replace(' ', "-") };
                if seen_keys.insert(key.clone()) {
                    features.push(FeatureResponse { key: key.clone(), label: label.clone(), icon: None, tooltip: None });
                }
            }
        }

        let as_string = data.to_string().to_ascii_lowercase();
        if controller_level.is_none() {
            if as_string.contains("full controller") || as_string.contains("full controller support") {
                controller_level = Some("Full".to_string());
                seen_keys.insert("controller-support".to_string());
            } else if as_string.contains("partial controller") || as_string.contains("partial controller support") {
                controller_level = Some("Partial".to_string());
                seen_keys.insert("controller-support".to_string());
            }
        }

        if (as_string.contains("workshop") || as_string.contains("steam workshop")) && !as_string.contains("trading card") && !as_string.contains("trading cards") {
            if seen_keys.insert("workshop".to_string()) {
                features.push(FeatureResponse { key: "workshop".to_string(), label: "Steam Workshop".to_string(), icon: Some("workshop".to_string()), tooltip: None });
            }
        }

        if (as_string.contains("family sharing") || as_string.contains("family-share") || as_string.contains("family_share")) && !as_string.contains("trading card") && !as_string.contains("trading cards") {
            if seen_keys.insert("family-sharing".to_string()) {
                features.push(FeatureResponse { key: "family-sharing".to_string(), label: "Family Sharing".to_string(), icon: Some("family".to_string()), tooltip: None });
            }
        }
    }

    if has_achievements.unwrap_or(false) {
        let tooltip = achievements_count.map(|c| format!("{} achievements", c));
        if seen_keys.insert("achievements".to_string()) {
            features.push(FeatureResponse { key: "achievements".to_string(), label: "Achievements".to_string(), icon: Some("trophy".to_string()), tooltip });
        }
    }

    if has_cloud_saves.unwrap_or(false) {
        if seen_keys.insert("cloud-saves".to_string()) {
            features.push(FeatureResponse { key: "cloud-saves".to_string(), label: "Cloud Saves".to_string(), icon: Some("cloud".to_string()), tooltip: cloud_details.clone() });
        }
    }

    if controller_level.is_none() {
        if let Some(ctrl) = controller_support.as_deref() {
            if !ctrl.trim().is_empty() {
                controller_level = Some(ctrl.to_string());
            }
        }
    }

    if let Some(ctrl_val) = controller_level {
        if seen_keys.insert("controller-support".to_string()) {
            let label = match ctrl_val.to_ascii_lowercase().as_str() {
                "full" | "full controller" | "full controller support" => "Full Controller Support".to_string(),
                "partial" | "partial controller" | "partial controller support" => "Partial Controller Support".to_string(),
                other if other.trim().is_empty() => "Controller Support".to_string(),
                other => {
                    let mut out = String::new();
                    for w in other.split_whitespace() {
                        let mut chars = w.chars();
                        if let Some(first) = chars.next() {
                            out.push_str(&first.to_uppercase().to_string());
                            out.push_str(&chars.as_str().to_lowercase());
                            out.push(' ');
                        }
                    }
                    format!("{}Controller Support", out.trim())
                }
            };
            features.push(FeatureResponse { key: "controller-support".to_string(), label, icon: Some("gamepad".to_string()), tooltip: None });
        }
    }

    features
}
