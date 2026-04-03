use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

fn encode_steam_launch_options(launch_options: &str) -> String {
    url::form_urlencoded::byte_serialize(launch_options.as_bytes()).collect::<String>()
}

fn try_spawn_command(command: &str, args: &[&str]) -> Result<(), String> {
    Command::new(command)
        .args(args)
        .spawn()
        .map(|_| ())
        .map_err(|error| {
            let rendered_args = if args.is_empty() {
                String::new()
            } else {
                format!(" {}", args.join(" "))
            };
            format!("{command}{rendered_args}: {error}")
        })
}

fn sanitize_desktop_shortcut_name(name: &str) -> String {
    let mut sanitized = String::new();
    for character in name.chars() {
        if character.is_ascii_alphanumeric() || matches!(character, ' ' | '-' | '_') {
            sanitized.push(character);
        }
    }

    let trimmed = sanitized.trim();
    if trimmed.is_empty() {
        return String::from("Steam Game");
    }

    trimmed.to_owned()
}

fn resolve_desktop_shortcuts_directory() -> Result<PathBuf, String> {
    let home_directory = std::env::var("HOME")
        .ok()
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var("USERPROFILE")
                .ok()
                .map(|value| value.trim().to_owned())
                .filter(|value| !value.is_empty())
                .map(PathBuf::from)
        })
        .ok_or_else(|| {
            String::from("Could not resolve user home directory for desktop shortcut")
        })?;

    let desktop_directory = home_directory.join("Desktop");
    if desktop_directory.is_dir() {
        return Ok(desktop_directory);
    }
    if fs::create_dir_all(&desktop_directory).is_ok() {
        return Ok(desktop_directory);
    }

    let fallback_directory = if cfg!(target_os = "windows") {
        home_directory
    } else if cfg!(target_os = "macos") {
        home_directory.join("Applications")
    } else {
        home_directory
            .join(".local")
            .join("share")
            .join("applications")
    };
    fs::create_dir_all(&fallback_directory).map_err(|error| {
        format!(
            "Could not create fallback shortcut directory at {}: {error}",
            fallback_directory.display()
        )
    })?;
    Ok(fallback_directory)
}

fn create_steam_game_desktop_shortcut(external_id: &str, game_name: &str) -> Result<(), String> {
    let app_id = external_id
        .parse::<u64>()
        .map_err(|_| String::from("Steam external_id must be a numeric app ID"))?;
    let shortcuts_directory = resolve_desktop_shortcuts_directory()?;
    let shortcut_name = sanitize_desktop_shortcut_name(game_name);

    #[cfg(target_os = "windows")]
    {
        let shortcut_path = shortcuts_directory.join(format!("{shortcut_name}.url"));
        let content = format!("[InternetShortcut]\r\nURL=steam://run/{app_id}\r\n");
        fs::write(&shortcut_path, content).map_err(|error| {
            format!(
                "Could not write desktop shortcut at {}: {error}",
                shortcut_path.display()
            )
        })?;
        return Ok(());
    }

    #[cfg(target_os = "macos")]
    {
        let shortcut_path = shortcuts_directory.join(format!("{shortcut_name}.webloc"));
        let content = format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>URL</key>
  <string>steam://run/{app_id}</string>
</dict>
</plist>
"#
        );
        fs::write(&shortcut_path, content).map_err(|error| {
            format!(
                "Could not write desktop shortcut at {}: {error}",
                shortcut_path.display()
            )
        })?;
        return Ok(());
    }

    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        let shortcut_path = shortcuts_directory.join(format!("{shortcut_name}.desktop"));
        let content = format!(
            "[Desktop Entry]\nType=Application\nVersion=1.0\nName={shortcut_name}\nExec=xdg-open steam://run/{app_id}\nIcon=steam\nTerminal=false\nCategories=Game;\nStartupNotify=true\n"
        );
        fs::write(&shortcut_path, content).map_err(|error| {
            format!(
                "Could not write desktop shortcut at {}: {error}",
                shortcut_path.display()
            )
        })?;

        let metadata = fs::metadata(&shortcut_path).map_err(|error| {
            format!(
                "Could not read desktop shortcut metadata at {}: {error}",
                shortcut_path.display()
            )
        })?;
        let mut permissions = metadata.permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&shortcut_path, permissions).map_err(|error| {
            format!(
                "Could not set executable permissions on desktop shortcut at {}: {error}",
                shortcut_path.display()
            )
        })?;

        return Ok(());
    }

    #[allow(unreachable_code)]
    Err(String::from(
        "Desktop shortcut creation is unsupported on this platform",
    ))
}

fn launch_steam_uri(uri: &str, action: &str) -> Result<(), String> {
    let install_action = action.eq_ignore_ascii_case("install");

    if cfg!(target_os = "windows") {
        let mut errors = Vec::new();

        if install_action {
            match try_spawn_command("cmd", &["/C", "start", "", "/MIN", "steam", "-silent", uri]) {
                Ok(()) => return Ok(()),
                Err(error) => errors.push(error),
            }
            let _ = try_spawn_command("cmd", &["/C", "start", "", "/MIN", "steam", "-silent"]);
            match try_spawn_command("cmd", &["/C", "start", "", "/MIN", uri]) {
                Ok(()) => return Ok(()),
                Err(error) => errors.push(error),
            }
        } else {
            match try_spawn_command("cmd", &["/C", "start", "", uri]) {
                Ok(()) => return Ok(()),
                Err(error) => errors.push(error),
            }
        }

        return Err(format!(
            "Failed to launch Steam URI '{uri}' on Windows. Attempts: {}",
            errors.join("; ")
        ));
    }

    if cfg!(target_os = "macos") {
        let mut errors = Vec::new();

        if install_action {
            let _ = try_spawn_command("open", &["-g", "-j", "-a", "Steam"]);
            match try_spawn_command("open", &["-g", uri]) {
                Ok(()) => return Ok(()),
                Err(error) => errors.push(error),
            }
        }

        match try_spawn_command("open", &[uri]) {
            Ok(()) => return Ok(()),
            Err(error) => errors.push(error),
        }

        return Err(format!(
            "Failed to launch Steam URI '{uri}' on macOS. Attempts: {}",
            errors.join("; ")
        ));
    }

    if cfg!(target_os = "linux") {
        let mut errors = Vec::new();

        if install_action {
            // Warm Steam in the background, then dispatch the URI via open commands.
            let _ = try_spawn_command("steam", &["-silent"]);
            let _ = try_spawn_command("steam-runtime", &["-silent"]);
            let _ = try_spawn_command("flatpak", &["run", "com.valvesoftware.Steam", "-silent"]);
        }

        match try_spawn_command("xdg-open", &[uri]) {
            Ok(()) => return Ok(()),
            Err(error) => errors.push(error),
        }

        match try_spawn_command("gio", &["open", uri]) {
            Ok(()) => return Ok(()),
            Err(error) => errors.push(error),
        }

        match try_spawn_command("steam", &[uri]) {
            Ok(()) => return Ok(()),
            Err(error) => errors.push(error),
        }

        match try_spawn_command("steam-runtime", &[uri]) {
            Ok(()) => return Ok(()),
            Err(error) => errors.push(error),
        }

        match try_spawn_command("flatpak", &["run", "com.valvesoftware.Steam", uri]) {
            Ok(()) => return Ok(()),
            Err(error) => errors.push(error),
        }

        match webbrowser::open(uri) {
            Ok(_) => return Ok(()),
            Err(error) => errors.push(format!("webbrowser::open {uri}: {error}")),
        }

        return Err(format!(
            "Could not open Steam URI '{uri}'. Make sure Steam is installed and available in PATH. Attempts: {}",
            errors.join("; ")
        ));
    }

    webbrowser::open(uri)
        .map(|_| ())
        .map_err(|error| format!("Failed to open Steam URI '{uri}': {error}"))
}

pub(crate) fn open_path_in_file_manager(path: &Path) -> Result<(), String> {
    let open_result = if cfg!(target_os = "windows") {
        Command::new("explorer").arg(path).spawn()
    } else if cfg!(target_os = "macos") {
        Command::new("open").arg(path).spawn()
    } else {
        Command::new("xdg-open").arg(path).spawn()
    };

    open_result
        .map(|_| ())
        .map_err(|error| format!("Failed to open path {}: {error}", path.display()))
}

pub(crate) fn create_provider_game_desktop_shortcut(
    provider: &str,
    external_id: &str,
    game_name: &str,
) -> Result<(), String> {
    match provider {
        "steam" => create_steam_game_desktop_shortcut(external_id, game_name),
        _ => Err(format!(
            "Provider '{provider}' is not supported for desktop shortcut creation"
        )),
    }
}

pub(crate) fn open_steam_game_recording_settings() -> Result<(), String> {
    let candidate_uris = [
        "steam://open/settings/gamerecording",
        "steam://settings/gamerecording",
        "steam://open/settings",
        "steam://settings",
    ];
    let mut errors = Vec::new();
    for uri in candidate_uris {
        match launch_steam_uri(uri, "open-settings") {
            Ok(()) => return Ok(()),
            Err(error) => errors.push(error),
        }
    }

    let help_url = "https://help.steampowered.com/en/";
    match webbrowser::open(help_url) {
        Ok(_) => Ok(()),
        Err(error) => {
            errors.push(format!("webbrowser::open {help_url}: {error}"));
            Err(format!(
                "Could not open Steam game recording settings. Attempts: {}",
                errors.join("; ")
            ))
        }
    }
}

pub(crate) fn open_provider_game_uri(
    provider: &str,
    external_id: &str,
    action: &str,
    launch_options: Option<&str>,
) -> Result<(), String> {
    match provider {
        "steam" => {
            let app_id = external_id
                .parse::<u64>()
                .map_err(|_| String::from("Steam external_id must be a numeric app ID"))?;
            let uri = match action {
                "play" => match launch_options {
                    Some(value) => {
                        let encoded_options = encode_steam_launch_options(value);
                        format!("steam://run/{app_id}//{encoded_options}/")
                    }
                    None => format!("steam://run/{app_id}"),
                },
                "install" => format!("steam://install/{app_id}"),
                "uninstall" => format!("steam://uninstall/{app_id}"),
                "validate" => format!("steam://validate/{app_id}"),
                "backup" => format!("steam://backup/{app_id}"),
                _ => return Err(String::from("Unsupported Steam action")),
            };

            launch_steam_uri(&uri, action)
        }
        _ => Err(format!(
            "Provider '{provider}' is not supported for action '{action}'"
        )),
    }
}
