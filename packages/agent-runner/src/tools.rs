// Tools layer — tool registry + parallel executor.
// Each tool invocation is an independent async operation.

use serde_json::Value;

pub async fn invoke(tool_id: &str, input: &Value) -> String {
    match tool_id {
        "http_client" => http_client(input).await,
        "web_search"  => web_search(input).await,
        "shell"       => shell(input).await,
        _             => format!("unknown tool: {tool_id}"),
    }
}

async fn http_client(input: &Value) -> String {
    let url = match input["url"].as_str() {
        Some(u) => u.to_string(),
        None    => return "http_client: missing url".into(),
    };
    let method = input["method"].as_str().unwrap_or("GET").to_uppercase();
    let client = reqwest::Client::new();
    let req = match method.as_str() {
        "POST" => client.post(&url).body(input["body"].to_string()),
        _      => client.get(&url),
    };
    match req.send().await {
        Ok(r)  => r.text().await.unwrap_or_else(|e| e.to_string()),
        Err(e) => format!("http_client error: {e}"),
    }
}

async fn web_search(input: &Value) -> String {
    let query = input["query"].as_str().unwrap_or("");
    let key   = std::env::var("BRAVE_API_KEY").unwrap_or_default();
    if key.is_empty() { return format!("(demo) search results for: {query}"); }
    let url = format!("https://api.search.brave.com/res/v1/web/search?q={}", urlencoding::encode(query));
    let client = reqwest::Client::new();
    match client.get(&url).header("X-Subscription-Token", &key).send().await {
        Ok(r)  => r.text().await.unwrap_or_default(),
        Err(e) => format!("web_search error: {e}"),
    }
}

async fn shell(input: &Value) -> String {
    if std::env::var("ALLOW_SHELL").as_deref() != Ok("true") {
        return "shell: disabled (set ALLOW_SHELL=true)".into();
    }
    let cmd = input["command"].as_str().unwrap_or("");
    match tokio::process::Command::new("sh").arg("-c").arg(cmd).output().await {
        Ok(out) => {
            let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
            let stderr = String::from_utf8_lossy(&out.stderr).into_owned();
            if stderr.is_empty() { stdout } else { format!("{stdout}\nstderr: {stderr}") }
        }
        Err(e) => format!("shell error: {e}"),
    }
}
