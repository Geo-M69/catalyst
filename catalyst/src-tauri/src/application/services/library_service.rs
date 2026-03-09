use crate::*;
use crate::application::error::AppResult;
use std::process::Command;

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GameStoreMetadataResponse {
	pub developers: Option<Vec<String>>,
	pub publishers: Option<Vec<String>>,
	pub franchise: Option<String>,
	pub release_date: Option<String>,
	pub short_description: Option<String>,
	pub header_image: Option<String>,
	pub has_achievements: Option<bool>,
	pub achievements_count: Option<i64>,
	pub has_cloud_saves: Option<bool>,
	pub cloud_details: Option<String>,
	pub controller_support: Option<String>,
	pub features: Option<Vec<FeatureResponse>>,
}

// FeatureResponse is defined in crate root (`lib.rs`) so it can be shared across responses.

pub(crate) fn get_library(state: &AppState) -> AppResult<LibraryResponse> {
	let connection = open_connection(&state.db_path)?;
	cleanup_expired_sessions(&connection)?;
	let user = get_authenticated_user(state, &connection)?;
	let games = list_games_by_user(&connection, &user.id)?;

	// (removed debug log)

	Ok(LibraryResponse {
		user_id: user.id,
		total: games.len(),
		games,
	})
}

pub(crate) fn sync_steam_library(state: &AppState) -> AppResult<SteamSyncResponse> {
	let connection = open_connection(&state.db_path)?;
	cleanup_expired_sessions(&connection)?;
	let user = get_authenticated_user(state, &connection)?;
	let client = build_http_client()?;
	let synced_games = sync_steam_games_for_user(
		&connection,
		&user,
		state.steam_api_key.as_deref(),
		state.steam_local_install_detection,
		state.steam_root_override.as_deref(),
		&client,
	)?;

	Ok(SteamSyncResponse {
		user_id: user.id,
		provider: String::from("steam"),
		synced_games,
	})
}

pub(crate) fn set_game_favorite(
	state: &AppState,
	provider: String,
	external_id: String,
	favorite: bool,
) -> AppResult<()> {
	let connection = open_connection(&state.db_path)?;
	cleanup_expired_sessions(&connection)?;
	let user = get_authenticated_user(state, &connection)?;
	let (provider, external_id) = normalize_game_identity_input(&provider, &external_id)?;
	ensure_owned_game_exists(&connection, &user.id, &provider, &external_id)?;

	if favorite {
		upsert_game_favorite(&connection, &user.id, &provider, &external_id)?;
	} else {
		remove_game_favorite(&connection, &user.id, &provider, &external_id)?;
	}

	Ok(())
}

pub(crate) fn list_steam_downloads(state: &AppState) -> AppResult<Vec<SteamDownloadProgressResponse>> {
	let owned_games_by_app_id = match open_connection(&state.db_path) {
		Ok(connection) => {
			if let Err(error) = cleanup_expired_sessions(&connection) {
				eprintln!(
					"Steam download tracking: failed to cleanup expired sessions ({error}); continuing without ownership map."
				);
				HashMap::new()
			} else {
				match get_authenticated_user(state, &connection) {
					Ok(user) => match load_owned_steam_games_by_app_id(&connection, &user.id) {
						Ok(games) => games,
						Err(error) => {
							eprintln!(
								"Steam download tracking: could not load owned Steam games ({error}); continuing without ownership map."
							);
							HashMap::new()
						}
					},
					Err(error) => {
						eprintln!(
							"Steam download tracking: could not resolve authenticated user metadata ({error}); continuing without ownership map."
						);
						HashMap::new()
					}
				}
			}
		}
		Err(error) => {
			eprintln!(
				"Steam download tracking: could not open app database ({error}); continuing without ownership map."
			);
			HashMap::new()
		}
	};

	let steam_roots = resolve_steam_root_paths(state.steam_root_override.as_deref());
	if steam_roots.is_empty() {
		return Ok(Vec::new());
	}
	let mut downloads = Vec::new();
	let mut seen_external_ids = HashSet::new();

	for steam_root in steam_roots {
		let steamapps_directories = match resolve_steamapps_directories(&steam_root) {
			Ok(paths) => paths,
			Err(error) => {
				eprintln!(
					"Could not resolve Steam library paths from root {}: {}",
					steam_root.display(),
					error
				);
				continue;
			}
		};
		for steamapps_directory in steamapps_directories {
			if let Err(error) = collect_steam_download_progress_from_steamapps_dir(
				&steamapps_directory,
				&owned_games_by_app_id,
				&mut seen_external_ids,
				&mut downloads,
			) {
				eprintln!(
					"Could not read Steam download progress from {}: {}",
					steamapps_directory.display(),
					error
				);
			}
		}
	}

	downloads.sort_by(|left, right| {
		left.name
			.to_ascii_lowercase()
			.cmp(&right.name.to_ascii_lowercase())
	});
	Ok(downloads)
}

pub(crate) fn get_game_store_metadata(
	state: &AppState,
	provider: String,
	external_id: String,
) -> AppResult<GameStoreMetadataResponse> {
	let connection = open_connection(&state.db_path)?;
	cleanup_expired_sessions(&connection)?;
	let user = get_authenticated_user(state, &connection)?;
	let (normalized_provider, normalized_external_id) =
		normalize_game_identity_input(&provider, &external_id)?;
	ensure_owned_game_exists(
		&connection,
		&user.id,
		&normalized_provider,
		&normalized_external_id,
	)?;

	// Only Steam is supported for rich store metadata at the moment
	if normalized_provider != "steam" {
		return Ok(GameStoreMetadataResponse {
			developers: None,
			publishers: None,
			franchise: None,
			release_date: None,
			short_description: None,
			header_image: None,
			has_achievements: None,
			achievements_count: None,
			has_cloud_saves: None,
			cloud_details: None,
			controller_support: None,
			features: None,
		});
	}

	let app_id = match normalized_external_id.parse::<u64>() {
		Ok(v) => v,
		Err(_) => {
			return Ok(GameStoreMetadataResponse {
				developers: None,
				publishers: None,
				franchise: None,
				release_date: None,
				short_description: None,
				header_image: None,
				has_achievements: None,
				achievements_count: None,
				has_cloud_saves: None,
				cloud_details: None,
				controller_support: None,
				features: None,
			})
		}
	};

	let stale_before = Utc::now() - ChronoDuration::hours(STEAM_APP_DETAILS_CACHE_TTL_HOURS);

	let mut response = GameStoreMetadataResponse {
		developers: None,
		publishers: None,
		franchise: None,
		release_date: None,
		short_description: None,
		header_image: None,
		has_achievements: None,
		achievements_count: None,
		has_cloud_saves: None,
		cloud_details: None,
		controller_support: None,
		features: None,
	};

	// Keep a reference to parsed store data (if available) to build normalized feature list later.
	let mut maybe_data: Option<serde_json::Value> = None;

	if let Ok(Some(cached)) = find_cached_steam_app_details(&connection, app_id, stale_before) {
		if let Some(data) = cached.get("data") {

			// capture parsed data for normalized feature building
			maybe_data = Some(data.clone());
			if let Some(devs) = data.get("developers").and_then(serde_json::Value::as_array) {
				let mut out: Vec<String> = Vec::new();
				for d in devs {
					if let Some(s) = d.as_str() {
						out.push(s.to_owned());
					}
				}
				if !out.is_empty() {
					response.developers = Some(out);
				}
			}
			if let Some(pubs) = data.get("publishers").and_then(serde_json::Value::as_array) {
				let mut out: Vec<String> = Vec::new();
				for p in pubs {
					if let Some(s) = p.as_str() {
						out.push(s.to_owned());
					}
				}
				if !out.is_empty() {
					response.publishers = Some(out);
				}
			}
									// franchise: prefer `franchise`, fall back to `series` array
									response.franchise = data
										.get("franchise")
										.and_then(serde_json::Value::as_str)
										.map(|s| s.to_owned())
										.or_else(|| {
											data.get("series").and_then(|v| v.as_array()).map(|arr| {
												arr.iter()
													.filter_map(|s| s.as_str())
													.collect::<Vec<_>>()
													.join(", ")
											})
										});

									// release_date: try nested `release_date.date`, then plain string fallback
									response.release_date = data
										.get("release_date")
										.and_then(|v| v.get("date"))
										.and_then(serde_json::Value::as_str)
										.map(|s| s.to_owned())
										.or_else(|| data.get("release_date").and_then(serde_json::Value::as_str).map(|s| s.to_owned()));
			if let Some(sd) = data.get("short_description").and_then(serde_json::Value::as_str) {
				response.short_description = Some(sd.to_owned());
			}
			if let Some(h) = data.get("header_image").and_then(serde_json::Value::as_str) {
				response.header_image = Some(h.to_owned());
			}
		}
	}

	// If no cached details were found, attempt a best-effort live fetch from the Steam Store
	if response.short_description.is_none() || response.developers.is_none() {
		// Prefer using steamcmd if available for an exact client-style appinfo
		if let Ok(output) = Command::new("bash").arg("-lc").arg(format!("steamcmd +login anonymous +app_info_print {} +quit", app_id)).output() {
			if output.status.success() {
				if let Ok(text) = String::from_utf8(output.stdout) {
					// Simple VDF-like key/value extraction: "key" "value"
					let re = regex::Regex::new(r#"\"([^\"]+)\"\s+\"([^\"]*)\""#).unwrap();
					let mut map: std::collections::HashMap<String, String> = std::collections::HashMap::new();
					for cap in re.captures_iter(&text) {
						map.insert(cap[1].to_string(), cap[2].to_string());
					}
										// populate response when available
										if response.developers.is_none() {
											if let Some(dev) = map.get("developer") {
												response.developers = Some(vec![dev.to_string()]);
											}
										}
										if response.publishers.is_none() {
											if let Some(pubr) = map.get("publisher") {
												response.publishers = Some(vec![pubr.to_string()]);
											}
										}
										if response.short_description.is_none() {
											if let Some(sd) = map.get("short_description") {
												response.short_description = Some(sd.to_string());
											}
										}
										if response.header_image.is_none() {
											if let Some(h) = map.get("header_image") {
												response.header_image = Some(h.to_string());
											}
										}
										if response.franchise.is_none() {
											if let Some(fr) = map.get("franchise") {
												response.franchise = Some(fr.to_string());
											}
										}
										// Build a minimal JSON details object to cache so downstream callers can reuse it
										let mut obj = serde_json::Map::new();
										let mut data_map = serde_json::Map::new();
										if let Some(dev) = map.get("developer") {
											data_map.insert("developers".to_string(), serde_json::Value::Array(vec![serde_json::Value::String(dev.to_string())]));
										}
										if let Some(pubr) = map.get("publisher") {
											data_map.insert("publishers".to_string(), serde_json::Value::Array(vec![serde_json::Value::String(pubr.to_string())]));
										}
										if let Some(sd) = map.get("short_description") {
											data_map.insert("short_description".to_string(), serde_json::Value::String(sd.to_string()));
										}
										if let Some(h) = map.get("header_image") {
											data_map.insert("header_image".to_string(), serde_json::Value::String(h.to_string()));
										}
										if let Some(fr) = map.get("franchise") {
											data_map.insert("franchise".to_string(), serde_json::Value::String(fr.to_string()));
										}
										obj.insert("data".to_string(), serde_json::Value::Object(data_map));
										obj.insert("success".to_string(), serde_json::Value::Bool(true));
										let entry = serde_json::Value::Object(obj);
										let _ = crate::cache_steam_app_details(&connection, app_id, &entry);
										// also expose parsed JSON data for later normalized feature building
										if let Some(d) = entry.get("data") {
											maybe_data = Some(d.clone());
										}
										// If we got any meaningful value, skip the HTTP store fetch.
										if response.short_description.is_some() || response.developers.is_some() || response.publishers.is_some() {
											// proceed to feature inference later; we have cached details now
										} else {
											// fall through to HTTP fetch below
										}
				}
			}
		}
		if let Ok(client) = crate::build_http_client() {
			// If steamcmd already provided useful fields, skip the HTTP store fetch.
			if response.short_description.is_some() || response.developers.is_some() || response.publishers.is_some() {
				// skip HTTP fetch: we prefer steamcmd results when present
			} else {
				let mut request_url = match url::Url::parse(crate::STEAM_APP_DETAILS_ENDPOINT) {
					Ok(u) => u,
					Err(_) => Url::parse("https://store.steampowered.com/api/appdetails").unwrap(),
				};
				// append query
				request_url.query_pairs_mut().append_pair("appids", &app_id.to_string()).append_pair("l", "english");
				if let Ok(resp) = client.get(request_url).send() {
					if resp.status().is_success() {
						if let Ok(payload) = resp.json::<serde_json::Value>() {
							if let Some(entry) = payload.get(&app_id.to_string()) {
								if entry.get("success").and_then(|v| v.as_bool()).unwrap_or(false) {
									if let Some(data) = entry.get("data") {
										// capture parsed data for normalized feature building
										maybe_data = Some(data.clone());
										let _ = crate::cache_steam_app_details(&connection, app_id, data);
										// infer features similar to cache_steam_app_details implementation
										let has_achievements = data.get("achievements").is_some();
										let has_cloud = data
											.get("cloud")
											.and_then(|v| v.get("enabled").and_then(serde_json::Value::as_bool))
											.unwrap_or_else(|| data.get("cloud").is_some());
										let mut controller_support: Option<String> = None;
										if let Some(categories) = data.get("categories").and_then(serde_json::Value::as_array) {
											for cat in categories {
												if let Some(desc) = cat.get("description").and_then(serde_json::Value::as_str) {
													let lowered = desc.to_ascii_lowercase();
													if lowered.contains("full controller") || lowered.contains("full controller support") {
														controller_support = Some(String::from("Full"));
														break;
													}
													if lowered.contains("partial controller") || lowered.contains("partial controller support") {
														controller_support = Some(String::from("Partial"));
														break;
													}
												}
											}
										}
										if controller_support.is_none() {
											if let Some(cs) = data.get("controller_support").and_then(serde_json::Value::as_str) {
												controller_support = Some(cs.to_owned());
											} else if let Some(cs) = data.get("controller_supports").and_then(serde_json::Value::as_str) {
												controller_support = Some(cs.to_owned());
											}
										}
										let _ = crate::cache_steam_app_features(&connection, app_id, has_achievements, None, has_cloud, None, controller_support.as_deref());

										// apply freshly fetched data to response
										if let Some(devs) = data.get("developers").and_then(|v| v.as_array()) {
											let mut out: Vec<String> = Vec::new();
											for d in devs {
												if let Some(s) = d.as_str() {
													out.push(s.to_owned());
												}
											}
											if !out.is_empty() {
												response.developers = Some(out);
											}
										}
										if let Some(pubs) = data.get("publishers").and_then(|v| v.as_array()) {
											let mut out: Vec<String> = Vec::new();
											for p in pubs {
												if let Some(s) = p.as_str() {
													out.push(s.to_owned());
												}
											}
											if !out.is_empty() {
												response.publishers = Some(out);
											}
										}
										if let Some(fr) = data.get("franchise").and_then(serde_json::Value::as_str) {
											response.franchise = Some(fr.to_owned());
										}
										if let Some(rel) = data.get("release_date").and_then(|v| v.get("date")).and_then(serde_json::Value::as_str) {
											response.release_date = Some(rel.to_owned());
										}
										if let Some(sd) = data.get("short_description").and_then(serde_json::Value::as_str) {
											response.short_description = Some(sd.to_owned());
										}
										if let Some(h) = data.get("header_image").and_then(serde_json::Value::as_str) {
											response.header_image = Some(h.to_owned());
										}
									}
								}
							}
						}
						}
					}
				}
			}
	}

	if let Ok(Some((has_ach, ach_count_opt, has_cloud, cloud_details_opt, controller_opt))) =
		find_cached_steam_app_features(&connection, app_id, stale_before)
	{
		response.has_achievements = Some(has_ach);
		response.achievements_count = ach_count_opt;
		response.has_cloud_saves = Some(has_cloud);
		response.cloud_details = cloud_details_opt;
		response.controller_support = controller_opt;
	}

	// Build a normalized features list using parsed store data and inferred flags.
	{
		let mut features: Vec<FeatureResponse> = Vec::new();

		if let Some(ref data) = maybe_data {
			// Categories mapping (Steam often lists these on the right)
			if let Some(categories) = data.get("categories").and_then(serde_json::Value::as_array) {
				let mut seen_keys: std::collections::HashSet<String> = std::collections::HashSet::new();
				// helper to canonicalize description to a preferred feature key/label
				let canonical_from_desc = |desc: &str| -> Option<(String, String)> {
					let lowered = desc.to_ascii_lowercase();
					if lowered.contains("remote play together") || lowered.contains("remote play") {
						// prefer showing Family Sharing instead of Remote Play Together per UX preference
						return Some(("family-sharing".to_string(), "Family Sharing".to_string()));
					}
					if lowered.contains("steam cloud") || lowered.contains("steam cloud saves") || lowered.contains("cloud saves") || lowered == "cloud" {
						return Some(("cloud-saves".to_string(), "Cloud Saves".to_string()));
					}
					// suppress Trading Cards entries — they are redundant in our UI
					if lowered.contains("trading card") || lowered.contains("trading cards") {
						return None;
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
					// Suppress explicit Trading Cards category by returning a skip marker
					if lowered.contains("trading card") || lowered.contains("trading cards") {
						return Some(("__skip__".to_string(), "".to_string()));
					}
					None
				};
				for cat in categories {
					let id_opt = cat.get("id").and_then(|v| v.as_u64());
					let desc_opt = cat.get("description").and_then(serde_json::Value::as_str).map(|s| s.to_string());
					if let Some(desc) = desc_opt.as_deref() {
						if let Some((key, label)) = canonical_from_desc(desc) {
							// allow canonical helper to mark items to skip (e.g., trading cards)
							if key == "__skip__" {
								continue;
							}
							if seen_keys.insert(key.clone()) {
								features.push(FeatureResponse { key: key.clone(), label: label.clone(), icon: None, tooltip: None });
							}
							// don't also add generic category-<id> when a canonical mapping applies
							continue;
						}
					}
					// no canonical mapping: include category id-based feature so raw ids are available in UI
					let label = desc_opt.clone().or_else(|| id_opt.map(|id| format!("Category {}", id))).unwrap_or_else(|| "Category".to_string());
					let key = if let Some(id) = id_opt { format!("category-{}", id) } else { label.to_ascii_lowercase().replace(' ', "-") };
					if seen_keys.insert(key.clone()) {
						features.push(FeatureResponse { key: key.clone(), label: label.clone(), icon: None, tooltip: None });
					}
				}
			}

			// Controller-specific strings (DualShock / DualSense) may appear in other fields
			let as_string = data.to_string().to_ascii_lowercase();
			if as_string.contains("dualshock") {
				features.push(FeatureResponse { key: "controller-dualshock".to_string(), label: "DualShock Support".to_string(), icon: Some("dualshock".to_string()), tooltip: None });
			}
			if as_string.contains("dualsense") {
				features.push(FeatureResponse { key: "controller-dualsense".to_string(), label: "DualSense Support".to_string(), icon: Some("dualsense".to_string()), tooltip: None });
			}
			// Steam Workshop
			if as_string.contains("workshop") || as_string.contains("steam workshop") {
				if !as_string.contains("trading card") && !as_string.contains("trading cards") {
					features.push(FeatureResponse { key: "workshop".to_string(), label: "Steam Workshop".to_string(), icon: Some("workshop".to_string()), tooltip: None });
				}
			}
			// Family Sharing eligibility
			if as_string.contains("family sharing") || as_string.contains("family-share") || as_string.contains("family_share") {
				if !as_string.contains("trading card") && !as_string.contains("trading cards") {
					features.push(FeatureResponse { key: "family-sharing".to_string(), label: "Family Sharing".to_string(), icon: Some("family".to_string()), tooltip: None });
				}
			}
		}

		// Achievements (use inferred/cached flag if present)
		if response.has_achievements.unwrap_or(false) {
			let tooltip = response.achievements_count.map(|c| format!("{} achievements", c));
			features.push(FeatureResponse { key: "achievements".to_string(), label: "Achievements".to_string(), icon: Some("trophy".to_string()), tooltip });
		}

		// Cloud saves
		if response.has_cloud_saves.unwrap_or(false) {
			features.push(FeatureResponse { key: "cloud-saves".to_string(), label: "Cloud Saves".to_string(), icon: Some("cloud".to_string()), tooltip: response.cloud_details.clone() });
		}

		// Controller support summary
		if let Some(ref ctrl) = response.controller_support {
			features.push(FeatureResponse { key: "controller-support".to_string(), label: format!("Controller: {}", ctrl), icon: Some("gamepad".to_string()), tooltip: None });
		}

		if !features.is_empty() {
			response.features = Some(features);
		}
	}

	Ok(response)
}

