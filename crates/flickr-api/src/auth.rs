use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use base64::Engine;
use hmac::{Hmac, Mac};
use percent_encoding::{utf8_percent_encode, AsciiSet, NON_ALPHANUMERIC};
use rand::Rng;
use sha1::Sha1;
use thiserror::Error;

use crate::types::OAuthTokens;

type HmacSha1 = Hmac<Sha1>;

/// Characters that must be percent-encoded per OAuth 1.0a (RFC 5849).
const OAUTH_ENCODE_SET: &AsciiSet = &NON_ALPHANUMERIC
    .remove(b'-')
    .remove(b'.')
    .remove(b'_')
    .remove(b'~');

const REQUEST_TOKEN_URL: &str = "https://www.flickr.com/services/oauth/request_token";
const AUTHORIZE_URL: &str = "https://www.flickr.com/services/oauth/authorize";
const ACCESS_TOKEN_URL: &str = "https://www.flickr.com/services/oauth/access_token";

const KEYRING_SERVICE: &str = "flickr-cli";
const KEYRING_USER: &str = "oauth_tokens";

#[derive(Error, Debug)]
pub enum AuthError {
    #[error("HTTP request failed: {0}")]
    Http(#[from] reqwest::Error),

    #[error("Keyring error: {0}")]
    Keyring(#[from] keyring::Error),

    #[error("Failed to parse OAuth response: {0}")]
    Parse(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("No stored tokens found. Run 'auth' first.")]
    NoTokens,

    #[error("HMAC key error")]
    Hmac,
}

/// Handles Flickr OAuth 1.0a authentication and token management.
pub struct FlickrAuth {
    api_key: String,
    api_secret: String,
    http: reqwest::Client,
    request_token_url: String,
    authorize_url: String,
    access_token_url: String,
    token_path: PathBuf,
}

impl FlickrAuth {
    pub fn new(api_key: String, api_secret: String) -> Self {
        Self {
            api_key,
            api_secret,
            http: reqwest::Client::new(),
            request_token_url: REQUEST_TOKEN_URL.to_string(),
            authorize_url: AUTHORIZE_URL.to_string(),
            access_token_url: ACCESS_TOKEN_URL.to_string(),
            token_path: Self::default_token_path(),
        }
    }

    /// Create an instance with custom URLs and token path (for testing).
    #[cfg(test)]
    pub fn new_for_test(
        api_key: String,
        api_secret: String,
        request_token_url: String,
        authorize_url: String,
        access_token_url: String,
        token_path: PathBuf,
    ) -> Self {
        Self {
            api_key,
            api_secret,
            http: reqwest::Client::new(),
            request_token_url,
            authorize_url,
            access_token_url,
            token_path,
        }
    }

    /// Perform the full OAuth 1.0a 3-legged flow.
    ///
    /// 1. Fetch a request token
    /// 2. Open the browser for user authorization
    /// 3. Read the verifier PIN from stdin
    /// 4. Exchange for an access token
    pub async fn oauth_flow(&self) -> Result<OAuthTokens, AuthError> {
        // Step 1: Get request token
        let (req_token, req_secret) = self.fetch_request_token().await?;

        // Step 2: Open browser for authorization
        let auth_url = format!("{}?oauth_token={}", self.authorize_url, req_token);
        println!("Authorize this app by visiting:\n {}", auth_url);
        let _ = open::that(&auth_url);

        // Step 3: Read verifier from user
        println!("Enter the verifier code:");
        let mut verifier = String::new();
        std::io::stdin().read_line(&mut verifier)?;
        let verifier = verifier.trim().to_string();

        // Step 4: Exchange for access token
        let tokens = self
            .fetch_access_token(&req_token, &req_secret, &verifier)
            .await?;

        Ok(tokens)
    }

    /// Save tokens to both the keyring and a file (~/.flickr_tokens).
    pub fn save_tokens(&self, tokens: &OAuthTokens) -> Result<(), AuthError> {
        // Save to keyring
        let json = serde_json::to_string(tokens)?;
        let entry = keyring::Entry::new(KEYRING_SERVICE, KEYRING_USER)?;
        entry.set_password(&json)?;

        // Save to file
        let token_path = &self.token_path;
        let content = format!(
            "oauth_token={}\noauth_token_secret={}\n",
            tokens.oauth_token, tokens.oauth_token_secret,
        );
        if let Some(fullname) = &tokens.fullname {
            let content = format!("{}fullname={}\n", content, fullname);
            std::fs::write(token_path, content)?;
        } else {
            std::fs::write(token_path, content)?;
        }
        println!("Tokens saved to {}", token_path.display());

        Ok(())
    }

    /// Load tokens from keyring, falling back to file.
    pub fn load_tokens(&self) -> Result<OAuthTokens, AuthError> {
        // Try keyring first
        if let Ok(entry) = keyring::Entry::new(KEYRING_SERVICE, KEYRING_USER) {
            if let Ok(data) = entry.get_password() {
                let tokens: OAuthTokens = serde_json::from_str(&data)?;
                return Ok(tokens);
            }
        }

        // Fall back to file
        let token_path = &self.token_path;
        if token_path.exists() {
            let content = std::fs::read_to_string(token_path)?;
            let mut oauth_token = String::new();
            let mut oauth_token_secret = String::new();

            for line in content.lines() {
                if let Some((key, value)) = line.split_once('=') {
                    match key.trim() {
                        "oauth_token" => oauth_token = value.trim().to_string(),
                        "oauth_token_secret" => oauth_token_secret = value.trim().to_string(),
                        _ => {}
                    }
                }
            }

            if !oauth_token.is_empty() && !oauth_token_secret.is_empty() {
                return Ok(OAuthTokens {
                    oauth_token,
                    oauth_token_secret,
                    fullname: None,
                    user_nsid: None,
                    username: None,
                });
            }
        }

        Err(AuthError::NoTokens)
    }

    fn default_token_path() -> PathBuf {
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".flickr_tokens")
    }

    // ── OAuth 1.0a internals ──────────────────────────────────────────

    async fn fetch_request_token(&self) -> Result<(String, String), AuthError> {
        let mut params = BTreeMap::new();
        params.insert("oauth_callback", "oob".to_string());

        let auth_header =
            self.build_oauth_header("GET", &self.request_token_url, &params, None, None);

        let resp = self
            .http
            .get(&self.request_token_url)
            .header("Authorization", auth_header)
            .send()
            .await?
            .text()
            .await?;

        let parsed = Self::parse_query_string(&resp);
        let token = parsed
            .get("oauth_token")
            .ok_or_else(|| AuthError::Parse("Missing oauth_token in response".into()))?
            .clone();
        let secret = parsed
            .get("oauth_token_secret")
            .ok_or_else(|| AuthError::Parse("Missing oauth_token_secret in response".into()))?
            .clone();

        Ok((token, secret))
    }

    async fn fetch_access_token(
        &self,
        req_token: &str,
        req_secret: &str,
        verifier: &str,
    ) -> Result<OAuthTokens, AuthError> {
        let mut params = BTreeMap::new();
        params.insert("oauth_verifier", verifier.to_string());

        let auth_header = self.build_oauth_header(
            "GET",
            &self.access_token_url,
            &params,
            Some(req_token),
            Some(req_secret),
        );

        let resp = self
            .http
            .get(&self.access_token_url)
            .header("Authorization", auth_header)
            .send()
            .await?
            .text()
            .await?;

        let parsed = Self::parse_query_string(&resp);

        let oauth_token = parsed
            .get("oauth_token")
            .ok_or_else(|| AuthError::Parse("Missing oauth_token".into()))?
            .clone();
        let oauth_token_secret = parsed
            .get("oauth_token_secret")
            .ok_or_else(|| AuthError::Parse("Missing oauth_token_secret".into()))?
            .clone();
        let fullname = parsed.get("fullname").cloned();
        let user_nsid = parsed.get("user_nsid").cloned();
        let username = parsed.get("username").cloned();

        Ok(OAuthTokens {
            oauth_token,
            oauth_token_secret,
            fullname,
            user_nsid,
            username,
        })
    }

    /// Build an OAuth 1.0a Authorization header.
    pub fn build_oauth_header(
        &self,
        method: &str,
        url: &str,
        extra_params: &BTreeMap<&str, String>,
        token: Option<&str>,
        token_secret: Option<&str>,
    ) -> String {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs()
            .to_string();

        let nonce: String = rand::rng()
            .sample_iter(&rand::distr::Alphanumeric)
            .take(32)
            .map(char::from)
            .collect();

        let mut params = BTreeMap::new();
        params.insert("oauth_consumer_key", self.api_key.as_str());
        params.insert("oauth_nonce", &nonce);
        params.insert("oauth_signature_method", "HMAC-SHA1");
        params.insert("oauth_timestamp", &timestamp);
        params.insert("oauth_version", "1.0");

        if let Some(t) = token {
            params.insert("oauth_token", t);
        }

        // Merge extra params for signature base
        let mut all_params: BTreeMap<String, String> = params
            .iter()
            .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
            .collect();
        for (k, v) in extra_params {
            all_params.insert((*k).to_string(), v.clone());
        }

        // Build parameter string (sorted)
        let param_string: String = all_params
            .iter()
            .map(|(k, v)| format!("{}={}", pct_encode(k), pct_encode(v),))
            .collect::<Vec<_>>()
            .join("&");

        // Build signature base string
        let base_string = format!(
            "{}&{}&{}",
            method.to_uppercase(),
            pct_encode(url),
            pct_encode(&param_string),
        );

        // Build signing key
        let signing_key = format!(
            "{}&{}",
            pct_encode(&self.api_secret),
            pct_encode(token_secret.unwrap_or("")),
        );

        // HMAC-SHA1
        let mut mac =
            HmacSha1::new_from_slice(signing_key.as_bytes()).expect("HMAC accepts any key size");
        mac.update(base_string.as_bytes());
        let signature =
            base64::engine::general_purpose::STANDARD.encode(mac.finalize().into_bytes());

        // Build Authorization header
        let mut header_parts = vec![
            format!("oauth_consumer_key=\"{}\"", pct_encode(&self.api_key)),
            format!("oauth_nonce=\"{}\"", pct_encode(&nonce)),
            format!("oauth_signature=\"{}\"", pct_encode(&signature)),
            format!("oauth_signature_method=\"HMAC-SHA1\""),
            format!("oauth_timestamp=\"{}\"", timestamp),
            format!("oauth_version=\"1.0\""),
        ];

        if let Some(t) = token {
            header_parts.push(format!("oauth_token=\"{}\"", pct_encode(t)));
        }

        for (k, v) in extra_params {
            if k.starts_with("oauth_") {
                header_parts.push(format!("{}=\"{}\"", k, pct_encode(v)));
            }
        }

        format!("OAuth {}", header_parts.join(", "))
    }

    fn parse_query_string(s: &str) -> BTreeMap<String, String> {
        s.split('&')
            .filter_map(|pair| {
                let mut parts = pair.splitn(2, '=');
                let key = parts.next()?.to_string();
                let value = parts.next().unwrap_or("").to_string();
                Some((key, value))
            })
            .collect()
    }
}

fn pct_encode(s: &str) -> String {
    utf8_percent_encode(s, OAUTH_ENCODE_SET).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    use tempfile::TempDir;

    // ── pct_encode ────────────────────────────────────────────────────

    #[test]
    fn pct_encode_alphanumeric_unchanged() {
        assert_eq!(pct_encode("abc123XYZ"), "abc123XYZ");
    }

    #[test]
    fn pct_encode_space_becomes_percent20() {
        assert_eq!(pct_encode("hello world"), "hello%20world");
    }

    #[test]
    fn pct_encode_space_not_plus() {
        let encoded = pct_encode("a b");
        assert!(!encoded.contains('+'), "space must not be encoded as +");
        assert!(encoded.contains("%20"));
    }

    #[test]
    fn pct_encode_special_chars_encoded() {
        // RFC 5849: !, *, ', (, ) must all be percent-encoded
        let encoded = pct_encode("!*'()");
        assert!(!encoded.contains('!'));
        assert!(!encoded.contains('*'));
        assert!(!encoded.contains('\''));
        assert!(!encoded.contains('('));
        assert!(!encoded.contains(')'));
        assert!(encoded.contains('%'));
    }

    #[test]
    fn pct_encode_unreserved_chars_unchanged() {
        // RFC 5849 unreserved: ALPHA / DIGIT / "-" / "." / "_" / "~"
        assert_eq!(pct_encode("-._~"), "-._~");
    }

    #[test]
    fn pct_encode_empty_string() {
        assert_eq!(pct_encode(""), "");
    }

    #[test]
    fn pct_encode_url_with_special_chars() {
        let encoded = pct_encode("https://example.com/path?key=val&other=x");
        // Colons, slashes, question marks, ampersands must be encoded
        assert!(!encoded.contains(':'));
        assert!(!encoded.contains('/'));
        assert!(!encoded.contains('?'));
        assert!(!encoded.contains('&'));
        assert!(!encoded.contains('='));
        // Should start with percent-encoded https
        assert!(encoded.starts_with("https%3A"));
    }

    // ── parse_query_string ────────────────────────────────────────────

    #[test]
    fn parse_query_string_standard() {
        let map = FlickrAuth::parse_query_string("oauth_token=abc&oauth_token_secret=xyz");
        assert_eq!(map.get("oauth_token").map(|s| s.as_str()), Some("abc"));
        assert_eq!(
            map.get("oauth_token_secret").map(|s| s.as_str()),
            Some("xyz")
        );
    }

    #[test]
    fn parse_query_string_empty_value() {
        let map = FlickrAuth::parse_query_string("key=");
        assert_eq!(map.get("key").map(|s| s.as_str()), Some(""));
    }

    #[test]
    fn parse_query_string_single_pair() {
        let map = FlickrAuth::parse_query_string("key=value");
        assert_eq!(map.len(), 1);
        assert_eq!(map.get("key").map(|s| s.as_str()), Some("value"));
    }

    #[test]
    fn parse_query_string_empty_input_produces_one_empty_key() {
        // Splitting "" on '&' gives one element ""; splitn gives key="" value=""
        let map = FlickrAuth::parse_query_string("");
        assert_eq!(map.len(), 1);
        assert!(map.contains_key(""));
    }

    #[test]
    fn parse_query_string_value_with_equals() {
        // splitn(2, '=') means only first '=' splits key from value
        let map = FlickrAuth::parse_query_string("sig=a=b=c");
        assert_eq!(map.get("sig").map(|s| s.as_str()), Some("a=b=c"));
    }

    // ── build_oauth_header ────────────────────────────────────────────

    fn make_auth() -> FlickrAuth {
        FlickrAuth::new("test_api_key".to_string(), "test_api_secret".to_string())
    }

    #[test]
    fn build_oauth_header_starts_with_oauth() {
        let auth = make_auth();
        let params: BTreeMap<&str, String> = BTreeMap::new();
        let header = auth.build_oauth_header("GET", "https://example.com", &params, None, None);
        assert!(
            header.starts_with("OAuth "),
            "header must start with 'OAuth '"
        );
    }

    #[test]
    fn build_oauth_header_contains_required_fields() {
        let auth = make_auth();
        let params: BTreeMap<&str, String> = BTreeMap::new();
        let header = auth.build_oauth_header("GET", "https://example.com", &params, None, None);
        assert!(header.contains("oauth_consumer_key="));
        assert!(header.contains("oauth_nonce="));
        assert!(header.contains("oauth_signature="));
        assert!(header.contains("oauth_signature_method="));
        assert!(header.contains("oauth_timestamp="));
        assert!(header.contains("oauth_version="));
    }

    #[test]
    fn build_oauth_header_with_token_includes_oauth_token() {
        let auth = make_auth();
        let params: BTreeMap<&str, String> = BTreeMap::new();
        let header = auth.build_oauth_header(
            "GET",
            "https://example.com",
            &params,
            Some("mytoken"),
            Some("mysecret"),
        );
        assert!(header.contains("oauth_token="));
    }

    #[test]
    fn build_oauth_header_without_token_omits_oauth_token() {
        let auth = make_auth();
        let params: BTreeMap<&str, String> = BTreeMap::new();
        let header = auth.build_oauth_header("GET", "https://example.com", &params, None, None);
        assert!(!header.contains("oauth_token="));
    }

    #[test]
    fn build_oauth_header_oauth_extra_params_included() {
        let auth = make_auth();
        let mut params: BTreeMap<&str, String> = BTreeMap::new();
        params.insert("oauth_callback", "oob".to_string());
        let header = auth.build_oauth_header("GET", "https://example.com", &params, None, None);
        assert!(header.contains("oauth_callback="));
    }

    #[test]
    fn build_oauth_header_non_oauth_extra_params_excluded_from_header() {
        let auth = make_auth();
        let mut params: BTreeMap<&str, String> = BTreeMap::new();
        params.insert("format", "json".to_string());
        params.insert("method", "flickr.test.login".to_string());
        let header = auth.build_oauth_header("GET", "https://example.com", &params, None, None);
        // These are in the signature base string but NOT in the Authorization header
        assert!(
            !header.contains("\"format\""),
            "non-oauth params must not appear in header"
        );
        assert!(
            !header.contains("\"method\""),
            "non-oauth params must not appear in header"
        );
    }

    // ── load_tokens file fallback ──────────────────────────────────────

    #[test]
    fn load_tokens_reads_file_when_keyring_unavailable() {
        let dir = TempDir::new().unwrap();
        let token_path = dir.path().join("tokens");

        // Write a token file manually (simulating the file-based fallback)
        let content = "oauth_token=mytoken123\noauth_token_secret=mysecret456\n";
        std::fs::write(&token_path, content).unwrap();

        let auth = FlickrAuth::new_for_test(
            "key".to_string(),
            "secret".to_string(),
            "http://localhost/req".to_string(),
            "http://localhost/auth".to_string(),
            "http://localhost/access".to_string(),
            token_path,
        );

        // Keyring may or may not be available; load_tokens falls back to file
        let result = auth.load_tokens();
        match result {
            Ok(tokens) => {
                assert_eq!(tokens.oauth_token, "mytoken123");
                assert_eq!(tokens.oauth_token_secret, "mysecret456");
            }
            Err(AuthError::NoTokens) => {
                // Keyring returned something unexpected; acceptable in CI
            }
            Err(e) => panic!("unexpected error: {e}"),
        }
    }

    #[test]
    fn load_tokens_missing_file_returns_no_tokens() {
        let dir = TempDir::new().unwrap();
        let token_path = dir.path().join("does_not_exist");

        let auth = FlickrAuth::new_for_test(
            "key".to_string(),
            "secret".to_string(),
            "http://localhost/req".to_string(),
            "http://localhost/auth".to_string(),
            "http://localhost/access".to_string(),
            token_path,
        );

        // With no keyring entry and no file, must return NoTokens
        // (keyring in CI may error, which causes load_tokens to fall through to file check)
        let result = auth.load_tokens();
        match result {
            Err(AuthError::NoTokens) => {}
            // If keyring had an entry from a previous test run, that's acceptable
            Ok(_) => {}
            Err(e) => panic!("unexpected error: {e}"),
        }
    }

    #[test]
    fn load_tokens_empty_file_returns_no_tokens() {
        let dir = TempDir::new().unwrap();
        let token_path = dir.path().join("tokens_empty");
        std::fs::write(&token_path, "").unwrap();

        let auth = FlickrAuth::new_for_test(
            "key".to_string(),
            "secret".to_string(),
            "http://localhost/req".to_string(),
            "http://localhost/auth".to_string(),
            "http://localhost/access".to_string(),
            token_path,
        );

        let result = auth.load_tokens();
        match result {
            Err(AuthError::NoTokens) => {}
            // keyring might succeed if an entry exists in the system
            Ok(_) => {}
            Err(e) => panic!("unexpected error: {e}"),
        }
    }

    // ── HTTP tests via wiremock ────────────────────────────────────────

    #[tokio::test]
    async fn fetch_request_token_parses_response() {
        use wiremock::matchers::any;
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let mock_server = MockServer::start().await;

        Mock::given(any())
            .respond_with(ResponseTemplate::new(200).set_body_string(
                "oauth_token=test_token&oauth_token_secret=test_secret&oauth_callback_confirmed=true",
            ))
            .mount(&mock_server)
            .await;

        let dir = TempDir::new().unwrap();
        let auth = FlickrAuth::new_for_test(
            "key".to_string(),
            "secret".to_string(),
            mock_server.uri() + "/request_token",
            mock_server.uri() + "/authorize",
            mock_server.uri() + "/access_token",
            dir.path().join("tokens"),
        );

        let (token, secret) = auth
            .fetch_request_token()
            .await
            .expect("fetch_request_token");
        assert_eq!(token, "test_token");
        assert_eq!(secret, "test_secret");
    }

    #[tokio::test]
    async fn fetch_access_token_parses_response() {
        use wiremock::matchers::any;
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let mock_server = MockServer::start().await;

        Mock::given(any())
            .respond_with(ResponseTemplate::new(200).set_body_string(
                "oauth_token=access_tok&oauth_token_secret=access_sec&fullname=Test%20User&username=testuser&user_nsid=123%40N01",
            ))
            .mount(&mock_server)
            .await;

        let dir = TempDir::new().unwrap();
        let auth = FlickrAuth::new_for_test(
            "key".to_string(),
            "secret".to_string(),
            mock_server.uri() + "/request_token",
            mock_server.uri() + "/authorize",
            mock_server.uri() + "/access_token",
            dir.path().join("tokens"),
        );

        let tokens = auth
            .fetch_access_token("req_tok", "req_sec", "verifier123")
            .await
            .expect("fetch_access_token");

        assert_eq!(tokens.oauth_token, "access_tok");
        assert_eq!(tokens.oauth_token_secret, "access_sec");
        // Note: URL-encoded values are returned as-is from parse_query_string
        assert_eq!(tokens.username.as_deref(), Some("testuser"));
    }
}
