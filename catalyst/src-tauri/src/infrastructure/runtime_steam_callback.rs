use regex::Regex;
use reqwest::blocking::Client;
use std::{
    collections::HashMap,
    io::{Read, Write},
    net::{TcpListener, TcpStream, ToSocketAddrs},
    thread,
    time::{Duration, Instant},
};
use url::Url;

pub(crate) fn resolve_steam_callback_public_host() -> String {
    let preferred_host = crate::STEAM_CALLBACK_PUBLIC_HOST.trim();
    if preferred_host.is_empty() {
        return String::from(crate::STEAM_CALLBACK_FALLBACK_HOST);
    }

    let can_resolve_preferred_host = (preferred_host, 0).to_socket_addrs().is_ok();
    if can_resolve_preferred_host {
        return preferred_host.to_owned();
    }

    eprintln!(
        "Steam callback host '{preferred_host}' could not be resolved. Falling back to {}.",
        crate::STEAM_CALLBACK_FALLBACK_HOST
    );
    String::from(crate::STEAM_CALLBACK_FALLBACK_HOST)
}

pub(crate) fn wait_for_steam_callback(
    listener: TcpListener,
    expected_state: &str,
    timeout: Duration,
    callback_public_host: &str,
) -> Result<HashMap<String, String>, String> {
    listener
        .set_nonblocking(true)
        .map_err(|error| format!("Failed to configure callback listener: {error}"))?;

    let deadline = Instant::now() + timeout;
    loop {
        if Instant::now() >= deadline {
            return Err(String::from(
                "Timed out waiting for Steam callback. Complete Steam sign-in in your browser and if Windows Firewall prompts for Catalyst, allow local/private access.",
            ));
        }

        match listener.accept() {
            Ok((mut stream, _)) => {
                let request_target = read_http_request_target(&mut stream)?;
                let callback_url = Url::parse(&format!("http://{callback_public_host}{request_target}"))
                    .map_err(|error| format!("Failed to parse callback URL: {error}"))?;
                let callback_params = callback_url
                    .query_pairs()
                    .map(|(key, value)| (key.to_string(), value.to_string()))
                    .collect::<HashMap<_, _>>();

                if callback_params.get("state").map(|value| value.as_str()) != Some(expected_state)
                {
                    let body = "<html><body><h2>Steam login failed</h2><p>State mismatch. Return to Catalyst and try again.</p></body></html>";
                    let _ = write_http_response(&mut stream, "400 Bad Request", body);
                    return Err(String::from("Steam callback state mismatch"));
                }

                let body = "<html><body><h2>Steam login complete</h2><p>You can close this tab and return to Catalyst.</p></body></html>";
                let _ = write_http_response(&mut stream, "200 OK", body);
                return Ok(callback_params);
            }
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                thread::sleep(Duration::from_millis(50));
            }
            Err(error) => return Err(format!("Failed while waiting for Steam callback: {error}")),
        }
    }
}

pub(crate) fn build_steam_authorization_url(return_to: &str, realm: &str) -> Result<String, String> {
    let mut url = Url::parse(crate::STEAM_OPENID_ENDPOINT)
        .map_err(|error| format!("Failed to parse Steam OpenID endpoint: {error}"))?;

    url.query_pairs_mut()
        .append_pair("openid.ns", "http://specs.openid.net/auth/2.0")
        .append_pair("openid.mode", "checkid_setup")
        .append_pair("openid.return_to", return_to)
        .append_pair("openid.realm", realm)
        .append_pair(
            "openid.identity",
            "http://specs.openid.net/auth/2.0/identifier_select",
        )
        .append_pair(
            "openid.claimed_id",
            "http://specs.openid.net/auth/2.0/identifier_select",
        );

    Ok(url.to_string())
}

pub(crate) fn verify_steam_openid_response(
    client: &Client,
    callback_params: &HashMap<String, String>,
) -> Result<bool, String> {
    let mut verification_form = callback_params
        .iter()
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect::<Vec<_>>();
    verification_form.retain(|(key, _)| key != "openid.mode");
    verification_form.push((
        String::from("openid.mode"),
        String::from("check_authentication"),
    ));

    let response = client
        .post(crate::STEAM_OPENID_ENDPOINT)
        .form(&verification_form)
        .send()
        .map_err(|error| format!("Steam OpenID verification request failed: {error}"))?;

    if !response.status().is_success() {
        return Err(format!(
            "Steam OpenID verification failed with status {}",
            response.status()
        ));
    }

    let body = response
        .text()
        .map_err(|error| format!("Failed to read Steam OpenID verification response: {error}"))?;
    Ok(body.contains("is_valid:true"))
}

pub(crate) fn extract_steam_id_from_callback_params(
    callback_params: &HashMap<String, String>,
) -> Result<String, String> {
    let claimed_id = callback_params
        .get("openid.claimed_id")
        .ok_or_else(|| String::from("Steam callback missing claimed ID"))?;

    let steam_id_pattern = Regex::new(r"/openid/id/(\d{17})$")
        .map_err(|error| format!("Failed to compile Steam ID regex: {error}"))?;
    steam_id_pattern
        .captures(claimed_id)
        .and_then(|capture| capture.get(1))
        .map(|matched| matched.as_str().to_owned())
        .ok_or_else(|| String::from("Steam callback returned an invalid claimed ID"))
}

fn read_http_request_target(stream: &mut TcpStream) -> Result<String, String> {
    let mut buffer = [0u8; 8192];
    let bytes_read = stream
        .read(&mut buffer)
        .map_err(|error| format!("Failed to read callback request: {error}"))?;
    if bytes_read == 0 {
        return Err(String::from("Steam callback request was empty"));
    }

    let request = String::from_utf8_lossy(&buffer[..bytes_read]);
    let request_line = request
        .lines()
        .next()
        .ok_or_else(|| String::from("Steam callback request line missing"))?;

    let mut parts = request_line.split_whitespace();
    let method = parts.next().unwrap_or_default();
    let target = parts.next().unwrap_or_default();

    if method != "GET" {
        return Err(format!("Steam callback used unsupported method: {method}"));
    }
    if target.is_empty() {
        return Err(String::from("Steam callback request target missing"));
    }

    Ok(target.to_owned())
}

fn write_http_response(stream: &mut TcpStream, status: &str, body: &str) -> Result<(), String> {
    let response = format!(
        "HTTP/1.1 {status}\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.as_bytes().len()
    );

    stream
        .write_all(response.as_bytes())
        .map_err(|error| format!("Failed to write callback response: {error}"))?;
    stream
        .flush()
        .map_err(|error| format!("Failed to flush callback response: {error}"))
}
