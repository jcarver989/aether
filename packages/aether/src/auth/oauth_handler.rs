use crate::auth::{AuthError, Result};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpListener;

/// Start a local callback server to capture the OAuth authorization code
///
/// Listens on the specified port and waits for the OAuth redirect.
/// Returns the authorization code from the callback URL.
pub async fn wait_for_callback(port: u16) -> Result<String> {
    let addr = format!("127.0.0.1:{port}");
    let listener = TcpListener::bind(&addr)
        .await
        .map_err(|e| AuthError::Io(format!("Failed to bind to {addr}: {e}")))?;

    let (mut socket, _) = listener
        .accept()
        .await
        .map_err(|e| AuthError::Io(format!("Failed to accept connection: {e}")))?;

    let mut reader = BufReader::new(&mut socket);
    let mut request_line = String::new();
    reader
        .read_line(&mut request_line)
        .await
        .map_err(|e| AuthError::Io(format!("Failed to read request: {e}")))?;

    // Parse the request line: GET /oauth2callback?code=XXX&state=YYY HTTP/1.1
    let code = parse_code_from_request(&request_line)?;

    // Send a success response
    let response = create_success_response();
    socket
        .write_all(response.as_bytes())
        .await
        .map_err(|e| AuthError::Io(format!("Failed to write response: {e}")))?;

    Ok(code)
}

/// Parse the authorization code from the HTTP request line
fn parse_code_from_request(request_line: &str) -> Result<String> {
    // Request format: GET /oauth2callback?code=XXX&state=YYY HTTP/1.1
    let parts: Vec<&str> = request_line.split_whitespace().collect();
    if parts.len() < 2 {
        return Err(AuthError::InvalidResponse(
            "Invalid HTTP request format".to_string(),
        ));
    }

    let path = parts[1];
    let query_start = path
        .find('?')
        .ok_or_else(|| AuthError::InvalidResponse("No query parameters in callback".to_string()))?;

    let query = &path[query_start + 1..];

    // Check for error in callback
    for param in query.split('&') {
        if let Some((key, value)) = param.split_once('=')
            && key == "error"
        {
            let error_desc = query
                .split('&')
                .find_map(|p| {
                    p.split_once('=')
                        .filter(|(k, _)| *k == "error_description")
                        .map(|(_, v)| urlencoding_decode(v))
                })
                .unwrap_or_else(|| value.to_string());
            return Err(AuthError::Other(format!("OAuth error: {error_desc}")));
        }
    }

    // Extract the code
    for param in query.split('&') {
        if let Some((key, value)) = param.split_once('=')
            && key == "code"
        {
            return Ok(urlencoding_decode(value));
        }
    }

    Err(AuthError::InvalidResponse(
        "No authorization code in callback".to_string(),
    ))
}

/// Simple URL decoding (handles %XX escapes)
fn urlencoding_decode(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();

    while let Some(c) = chars.next() {
        if c == '%' {
            let hex: String = chars.by_ref().take(2).collect();
            if let Ok(byte) = u8::from_str_radix(&hex, 16) {
                result.push(byte as char);
            } else {
                result.push('%');
                result.push_str(&hex);
            }
        } else if c == '+' {
            result.push(' ');
        } else {
            result.push(c);
        }
    }

    result
}

/// Create an HTML success response
fn create_success_response() -> String {
    let body = r#"<!DOCTYPE html>
<html>
<head>
    <title>Authentication Successful</title>
    <style>
        body {
            font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
            display: flex;
            justify-content: center;
            align-items: center;
            height: 100vh;
            margin: 0;
            background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
            color: white;
        }
        .container {
            text-align: center;
            padding: 40px;
            background: rgba(255,255,255,0.1);
            border-radius: 16px;
            backdrop-filter: blur(10px);
        }
        h1 { margin-bottom: 16px; }
        p { opacity: 0.9; }
    </style>
</head>
<body>
    <div class="container">
        <h1>Authentication Successful!</h1>
        <p>You can close this window and return to your terminal.</p>
    </div>
</body>
</html>"#;

    format!(
        "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        body.len(),
        body
    )
}

/// Open a URL in the default browser
pub fn open_browser(url: &str) -> Result<()> {
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(url)
            .spawn()
            .map_err(|e| AuthError::Other(format!("Failed to open browser: {e}")))?;
    }

    #[cfg(target_os = "linux")]
    {
        std::process::Command::new("xdg-open")
            .arg(url)
            .spawn()
            .map_err(|e| AuthError::Other(format!("Failed to open browser: {e}")))?;
    }

    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("cmd")
            .args(["/C", "start", url])
            .spawn()
            .map_err(|e| AuthError::Other(format!("Failed to open browser: {e}")))?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_code_from_valid_request() {
        let request = "GET /oauth2callback?code=4%2F0AYWS-abc123&state=verifier HTTP/1.1\r\n";
        let code = parse_code_from_request(request).unwrap();
        assert_eq!(code, "4/0AYWS-abc123");
    }

    #[test]
    fn parse_code_handles_plus_encoding() {
        let request = "GET /oauth2callback?code=hello+world&state=test HTTP/1.1\r\n";
        let code = parse_code_from_request(request).unwrap();
        assert_eq!(code, "hello world");
    }

    #[test]
    fn parse_code_returns_error_for_oauth_error() {
        let request =
            "GET /oauth2callback?error=access_denied&error_description=User+denied HTTP/1.1\r\n";
        let result = parse_code_from_request(request);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("User denied"));
    }

    #[test]
    fn parse_code_returns_error_for_missing_code() {
        let request = "GET /oauth2callback?state=verifier HTTP/1.1\r\n";
        let result = parse_code_from_request(request);
        assert!(result.is_err());
    }

    #[test]
    fn urlencoding_decode_handles_percent() {
        assert_eq!(urlencoding_decode("hello%20world"), "hello world");
        assert_eq!(urlencoding_decode("a%2Fb"), "a/b");
    }

    #[test]
    fn urlencoding_decode_handles_plus() {
        assert_eq!(urlencoding_decode("hello+world"), "hello world");
    }
}
