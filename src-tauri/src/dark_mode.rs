use serde::Deserialize;

#[derive(Deserialize)]
struct ThemeResponse {
    theme: String,
}

fn base_url() -> String {
    let port = crate::server_port();
    format!("http://127.0.0.1:{}", port)
}

#[tauri::command]
pub fn get_dark_mode() -> Result<bool, String> {
    let url = format!("{}/theme", base_url());
    let resp: ThemeResponse = reqwest::blocking::get(&url)
        .map_err(|e| format!("Failed to get theme: {}", e))?
        .json()
        .map_err(|e| format!("Failed to parse theme response: {}", e))?;
    Ok(resp.theme == "dark")
}

#[tauri::command]
pub fn set_dark_mode(enabled: bool) -> Result<(), String> {
    let route = if enabled { "dark" } else { "light" };
    let url = format!("{}/{}", base_url(), route);
    reqwest::blocking::get(&url)
        .map_err(|e| format!("Failed to set dark mode: {}", e))?;
    Ok(())
}
