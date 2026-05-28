//! HTTP probe helpers for the migrated command server.

#[cfg(target_os = "android")]
pub(crate) fn command_probe_addr(bind_addr: &str) -> String {
    if let Some(port) = bind_addr.strip_prefix("0.0.0.0:") {
        return format!("127.0.0.1:{port}");
    }
    if let Some(port) = bind_addr.strip_prefix("[::]:") {
        return format!("[::1]:{port}");
    }
    bind_addr.to_string()
}

#[cfg(target_os = "android")]
pub(crate) fn send_http_request(
    bind_addr: &str,
    method: &str,
    path: &str,
    body: Option<&str>,
) -> std::result::Result<String, String> {
    use std::io::{Read, Write};

    let connect_addr = command_probe_addr(bind_addr);
    let mut stream = std::net::TcpStream::connect(&connect_addr)
        .map_err(|err| format!("Failed to connect to migrated server {connect_addr}: {err}"))?;

    let _ = stream.set_read_timeout(Some(std::time::Duration::from_secs(3)));
    let _ = stream.set_write_timeout(Some(std::time::Duration::from_secs(3)));

    let body_text = body.unwrap_or("");
    let request = format!(
        "{method} {path} HTTP/1.1\r\nHost: {connect_addr}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body_text}",
        body_text.len()
    );

    stream
        .write_all(request.as_bytes())
        .map_err(|err| format!("Failed to write HTTP request to migrated server: {err}"))?;
    stream
        .flush()
        .map_err(|err| format!("Failed to flush HTTP request to migrated server: {err}"))?;

    let mut response_bytes = Vec::new();
    stream
        .read_to_end(&mut response_bytes)
        .map_err(|err| format!("Failed to read HTTP response from migrated server: {err}"))?;

    let response = String::from_utf8_lossy(&response_bytes);
    let mut sections = response.splitn(2, "\r\n\r\n");
    let headers = sections.next().unwrap_or("");
    let response_body = sections.next().unwrap_or("").to_string();
    let status_line = headers.lines().next().unwrap_or("HTTP/1.1 000");
    if !status_line.contains(" 200 ") {
        return Err(format!(
            "Migrated server returned non-200 status: {status_line}; body={response_body}"
        ));
    }
    Ok(response_body)
}
