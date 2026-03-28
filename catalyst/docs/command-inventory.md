# Tauri Command Inventory

Generated: 2026-03-28T04:27:21.837Z
Annotated commands found: 43
Registered in generate_handler: 0

## Registered commands


## Drift checks

- Annotated but not registered:
  - add_game_desktop_shortcut
  - add_game_to_collection
  - backup_game_files
  - browse_game_installed_files
  - clear_game_overlay_data
  - create_collection
  - delete_collection
  - get_game_achievements
  - get_game_activity_timeline
  - get_game_customization_artwork
  - get_game_dlc
  - get_game_friends_activity
  - get_game_install_size_estimate
  - get_game_installation_details
  - get_game_privacy_settings
  - get_game_properties_settings
  - get_game_review
  - get_game_screenshots
  - get_game_store_metadata
  - get_game_trading_cards
  - get_library
  - get_session
  - import_steam_collections
  - install_game
  - list_collections
  - list_game_compatibility_tools
  - list_game_install_locations
  - list_game_languages
  - list_game_versions_betas
  - list_steam_downloads
  - logout
  - open_game_recording_settings
  - play_game
  - rename_collection
  - set_game_favorite
  - set_game_privacy_settings
  - set_game_properties_settings
  - start_local_steam_scan
  - start_steam_auth
  - sync_steam_library
  - uninstall_game
  - validate_game_beta_access_code
  - verify_game_files
- Registered without #[tauri::command]: none

## Source coverage (annotated commands)

- add_game_desktop_shortcut (src-tauri/src/interface/tauri/commands/game_actions.rs:92)
- add_game_to_collection (src-tauri/src/interface/tauri/commands/collections.rs:52)
- backup_game_files (src-tauri/src/interface/tauri/commands/game_actions.rs:66)
- browse_game_installed_files (src-tauri/src/interface/tauri/commands/game_actions.rs:53)
- clear_game_overlay_data (src-tauri/src/interface/tauri/commands/game_settings.rs:71)
- create_collection (src-tauri/src/interface/tauri/commands/collections.rs:28)
- delete_collection (src-tauri/src/interface/tauri/commands/collections.rs:46)
- get_game_achievements (src-tauri/src/interface/tauri/commands/library.rs:92)
- get_game_activity_timeline (src-tauri/src/interface/tauri/commands/library.rs:77)
- get_game_customization_artwork (src-tauri/src/interface/tauri/commands/game_settings.rs:112)
- get_game_dlc (src-tauri/src/interface/tauri/commands/library.rs:122)
- get_game_friends_activity (src-tauri/src/interface/tauri/commands/library.rs:62)
- get_game_install_size_estimate (src-tauri/src/interface/tauri/commands/game_settings.rs:151)
- get_game_installation_details (src-tauri/src/interface/tauri/commands/game_settings.rs:138)
- get_game_privacy_settings (src-tauri/src/interface/tauri/commands/game_settings.rs:41)
- get_game_properties_settings (src-tauri/src/interface/tauri/commands/game_settings.rs:84)
- get_game_review (src-tauri/src/interface/tauri/commands/library.rs:137)
- get_game_screenshots (src-tauri/src/interface/tauri/commands/game_settings.rs:125)
- get_game_store_metadata (src-tauri/src/interface/tauri/commands/library.rs:49)
- get_game_trading_cards (src-tauri/src/interface/tauri/commands/library.rs:107)
- get_library (src-tauri/src/interface/tauri/commands/library.rs:17)
- get_session (src-tauri/src/interface/tauri/commands/auth.rs:11)
- import_steam_collections (src-tauri/src/interface/tauri/commands/steam.rs:39)
- install_game (src-tauri/src/interface/tauri/commands/game_actions.rs:21)
- list_collections (src-tauri/src/interface/tauri/commands/collections.rs:17)
- list_game_compatibility_tools (src-tauri/src/interface/tauri/commands/game_settings.rs:28)
- list_game_install_locations (src-tauri/src/interface/tauri/commands/game_settings.rs:164)
- list_game_languages (src-tauri/src/interface/tauri/commands/game_settings.rs:15)
- list_game_versions_betas (src-tauri/src/interface/tauri/commands/steam.rs:11)
- list_steam_downloads (src-tauri/src/interface/tauri/commands/library.rs:44)
- logout (src-tauri/src/interface/tauri/commands/auth.rs:6)
- open_game_recording_settings (src-tauri/src/interface/tauri/commands/game_actions.rs:105)
- play_game (src-tauri/src/interface/tauri/commands/game_actions.rs:6)
- rename_collection (src-tauri/src/interface/tauri/commands/collections.rs:35)
- set_game_favorite (src-tauri/src/interface/tauri/commands/library.rs:29)
- set_game_privacy_settings (src-tauri/src/interface/tauri/commands/game_settings.rs:54)
- set_game_properties_settings (src-tauri/src/interface/tauri/commands/game_settings.rs:97)
- start_local_steam_scan (src-tauri/src/interface/tauri/commands/library.rs:184)
- start_steam_auth (src-tauri/src/interface/tauri/commands/auth.rs:16)
- sync_steam_library (src-tauri/src/interface/tauri/commands/library.rs:24)
- uninstall_game (src-tauri/src/interface/tauri/commands/game_actions.rs:40)
- validate_game_beta_access_code (src-tauri/src/interface/tauri/commands/steam.rs:24)
- verify_game_files (src-tauri/src/interface/tauri/commands/game_actions.rs:79)
