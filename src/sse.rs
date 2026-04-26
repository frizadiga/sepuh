use futures::StreamExt;
use serde_json::Value;

/// Consumes a streaming HTTP response body framed as Server-Sent Events
/// (`data: {json}\n\n`) and invokes `on_payload` once for each parsed JSON
/// payload. Skips blank lines and `[DONE]` sentinels. Lines whose payload
/// fails JSON parsing are silently skipped (matches prior behaviour for the
/// Google streaming endpoint).
///
/// The response is expected to already have a successful status; callers
/// should check `response.status()` before invoking this helper if they want
/// custom error formatting (we leave that to callers because some APIs put
/// useful diagnostic text in the body that is consumed by reading bytes).
pub async fn for_each_sse_payload(
    response: reqwest::Response,
    mut on_payload: impl FnMut(Value) -> anyhow::Result<()>,
) -> anyhow::Result<()> {
    let mut stream = response.bytes_stream();
    let mut buf = String::new();

    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        buf.push_str(&String::from_utf8_lossy(&chunk));

        // Drain complete lines (terminated by '\n'). Normalize trailing '\r'.
        while let Some(nl) = buf.find('\n') {
            let mut line: String = buf.drain(..=nl).collect();
            line.pop(); // remove '\n'
            if line.ends_with('\r') {
                line.pop();
            }
            if let Some(payload) = parse_sse_line(&line) {
                on_payload(payload)?;
            }
        }
    }

    // Flush any trailing partial line (e.g. body without final '\n').
    if !buf.is_empty() {
        if let Some(payload) = parse_sse_line(buf.trim()) {
            on_payload(payload)?;
        }
        buf.clear();
    }

    Ok(())
}

/// Parses one SSE line into a JSON payload, or `None` if the line is empty,
/// a `[DONE]` sentinel, malformed, or a JSON-array stream framing line
/// (`[`, `]`, `,`).
fn parse_sse_line(line: &str) -> Option<Value> {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return None;
    }

    let payload = if let Some(rest) = trimmed.strip_prefix("data:") {
        let rest = rest.trim();
        if rest.is_empty() || rest == "[DONE]" {
            return None;
        }
        rest
    } else {
        // Tolerate the JSON-array streaming format used by some Google
        // endpoints when SSE isn't requested.
        let stripped = trimmed
            .trim_start_matches(['[', ',', ' '])
            .trim_end_matches([']', ',', ' ']);
        if stripped.is_empty() {
            return None;
        }
        stripped
    };

    serde_json::from_str(payload).ok()
}
