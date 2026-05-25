use serde_json::Value;

/// Parse a hook payload body and synthesize a short activity string.
/// Returns `None` when the body isn't JSON, isn't an object, or doesn't
/// carry a `tool_name` we recognise. Stop / SessionStart / etc. produce
/// `None` (no tool to describe).
pub fn extract_activity(body: &str) -> Option<String> {
    let v: Value = serde_json::from_str(body).ok()?;
    let tool_name = v.get("tool_name")?.as_str()?;
    let tool_input = v.get("tool_input").unwrap_or(&Value::Null);
    Some(format_activity(tool_name, tool_input))
}

/// Format a known Claude Code tool into a single-line activity string.
/// Unknown tools fall back to `"Running tool: <name>"`. Pure.
pub fn format_activity(tool_name: &str, tool_input: &Value) -> String {
    let s = |key: &str| tool_input.get(key).and_then(Value::as_str);
    match tool_name {
        "Bash" => s("command")
            .map(|c| format!("Running: {c}"))
            .unwrap_or_else(|| "Running bash".to_string()),
        "Read" => s("file_path")
            .map(|p| format!("Reading {p}"))
            .unwrap_or_else(|| "Reading".to_string()),
        "Edit" | "MultiEdit" => s("file_path")
            .map(|p| format!("Editing {p}"))
            .unwrap_or_else(|| "Editing".to_string()),
        "Write" => s("file_path")
            .map(|p| format!("Writing {p}"))
            .unwrap_or_else(|| "Writing".to_string()),
        "Grep" => s("pattern")
            .map(|p| format!("Searching: {p}"))
            .unwrap_or_else(|| "Searching".to_string()),
        "Glob" => s("pattern")
            .map(|p| format!("Finding: {p}"))
            .unwrap_or_else(|| "Finding".to_string()),
        "Task" => s("description")
            .map(|d| format!("Task: {d}"))
            .unwrap_or_else(|| "Running task".to_string()),
        "TodoWrite" => "Updating todos".to_string(),
        "WebFetch" => s("url")
            .map(|u| format!("Fetching: {u}"))
            .unwrap_or_else(|| "Fetching".to_string()),
        "WebSearch" => s("query")
            .map(|q| format!("Searching web: {q}"))
            .unwrap_or_else(|| "Searching web".to_string()),
        other => format!("Running tool: {other}"),
    }
}
