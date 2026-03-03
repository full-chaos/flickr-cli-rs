use std::collections::BTreeMap;
use std::path::Path;

use thiserror::Error;

use crate::auth::FlickrAuth;
use crate::types::{LoginResponse, OAuthTokens, Photo, PhotosResponse};

const FLICKR_BASE: &str = "https://api.flickr.com/services/rest";

#[derive(Error, Debug)]
pub enum ClientError {
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("API error: {0}")]
    Api(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

/// Flickr REST API client with OAuth 1.0a signing.
pub struct FlickrClient {
    auth: FlickrAuth,
    tokens: OAuthTokens,
    http: reqwest::Client,
    api_key: String,
    base_url: String,
}

impl FlickrClient {
    pub fn new(auth: FlickrAuth, tokens: OAuthTokens, api_key: String) -> Self {
        Self {
            auth,
            tokens,
            http: reqwest::Client::new(),
            api_key,
            base_url: FLICKR_BASE.to_string(),
        }
    }

    /// Create an instance with a custom base URL (for testing).
    #[cfg(test)]
    pub fn new_for_test(
        auth: FlickrAuth,
        tokens: OAuthTokens,
        api_key: String,
        base_url: String,
    ) -> Self {
        Self {
            auth,
            tokens,
            http: reqwest::Client::new(),
            api_key,
            base_url,
        }
    }

    /// Call flickr.test.login to get the authenticated user's ID.
    pub async fn get_user_id(&self) -> Result<String, ClientError> {
        let mut params = BTreeMap::new();
        params.insert("method", "flickr.test.login".to_string());
        params.insert("api_key", self.api_key.clone());
        params.insert("format", "json".to_string());
        params.insert("nojsoncallback", "1".to_string());

        let extra: BTreeMap<&str, String> = params.iter().map(|(k, v)| (*k, v.clone())).collect();

        let auth_header = self.auth.build_oauth_header(
            "GET",
            &self.base_url,
            &extra,
            Some(&self.tokens.oauth_token),
            Some(&self.tokens.oauth_token_secret),
        );

        let url = format!(
            "{}?{}",
            &self.base_url,
            params
                .iter()
                .map(|(k, v)| format!("{}={}", k, v))
                .collect::<Vec<_>>()
                .join("&")
        );

        let resp: LoginResponse = self
            .http
            .get(&url)
            .header("Authorization", auth_header)
            .send()
            .await?
            .json()
            .await?;

        Ok(resp.user.id)
    }

    /// Fetch all photos for a user with pagination and rate limiting.
    /// Returns up to `max_images` photos (or all if None).
    pub async fn fetch_all_photos(
        &self,
        user_id: &str,
        max_images: Option<usize>,
    ) -> Result<Vec<Photo>, ClientError> {
        let per_page = 500u32;
        let mut page = 1u32;
        let mut all_photos = Vec::new();

        loop {
            let mut params = BTreeMap::new();
            params.insert("method", "flickr.people.getPhotos".to_string());
            params.insert("api_key", self.api_key.clone());
            params.insert("user_id", user_id.to_string());
            params.insert("format", "json".to_string());
            params.insert("nojsoncallback", "1".to_string());
            params.insert("per_page", per_page.to_string());
            params.insert("page", page.to_string());
            params.insert(
                "extras",
                "date_taken,original_format,url_m,url_l,url_s".to_string(),
            );

            let extra: BTreeMap<&str, String> =
                params.iter().map(|(k, v)| (*k, v.clone())).collect();

            let auth_header = self.auth.build_oauth_header(
                "GET",
                &self.base_url,
                &extra,
                Some(&self.tokens.oauth_token),
                Some(&self.tokens.oauth_token_secret),
            );

            let url = format!(
                "{}?{}",
                &self.base_url,
                params
                    .iter()
                    .map(|(k, v)| format!("{}={}", k, v))
                    .collect::<Vec<_>>()
                    .join("&")
            );

            let resp: PhotosResponse = self
                .http
                .get(&url)
                .header("Authorization", auth_header)
                .send()
                .await?
                .json()
                .await?;

            let count = resp.photos.photo.len();
            all_photos.extend(resp.photos.photo);
            println!("Fetched {} photos on page {}", count, page);

            if let Some(max) = max_images {
                if all_photos.len() >= max {
                    all_photos.truncate(max);
                    break;
                }
            }

            if page >= resp.photos.pages {
                break;
            }

            page += 1;
            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        }

        Ok(all_photos)
    }

    /// Download a single photo to the given path. Skips if file exists.
    pub async fn download_photo(&self, photo: &Photo, dest: &Path) -> Result<bool, ClientError> {
        if dest.exists() {
            return Ok(false); // already exists
        }

        let Some(url) = photo.best_url() else {
            return Ok(false);
        };

        let bytes = self
            .http
            .get(url)
            .timeout(std::time::Duration::from_secs(30))
            .send()
            .await?
            .bytes()
            .await?;

        tokio::fs::write(dest, &bytes).await?;
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::FlickrAuth;
    use crate::types::OAuthTokens;
    use tempfile::TempDir;
    use wiremock::matchers::any;
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn make_tokens() -> OAuthTokens {
        OAuthTokens {
            oauth_token: "tok".to_string(),
            oauth_token_secret: "sec".to_string(),
            fullname: None,
            user_nsid: None,
            username: None,
        }
    }

    fn make_client(base_url: String) -> FlickrClient {
        let auth = FlickrAuth::new("test_key".to_string(), "test_secret".to_string());
        FlickrClient::new_for_test(auth, make_tokens(), "test_key".to_string(), base_url)
    }

    // ── get_user_id ───────────────────────────────────────────────────

    #[tokio::test]
    async fn get_user_id_returns_correct_id() {
        let mock_server = MockServer::start().await;

        Mock::given(any())
            .respond_with(ResponseTemplate::new(200).set_body_string(
                r#"{"user":{"id":"12345@N01","username":{"_content":"testuser"}},"stat":"ok"}"#,
            ))
            .mount(&mock_server)
            .await;

        let client = make_client(mock_server.uri());
        let id = client.get_user_id().await.expect("get_user_id");
        assert_eq!(id, "12345@N01");
    }

    // ── fetch_all_photos ──────────────────────────────────────────────

    fn make_photo_json(id: &str) -> String {
        format!(
            r#"{{
                "id": "{id}",
                "owner": "99@N01",
                "secret": "abc",
                "server": "1",
                "farm": 1,
                "title": "Photo {id}",
                "ispublic": 1,
                "isfriend": 0,
                "isfamily": 0
            }}"#
        )
    }

    fn make_photos_response(photos: &[&str], pages: u32) -> String {
        let photo_array: Vec<String> = photos.iter().map(|id| make_photo_json(id)).collect();
        let total = photos.len();
        format!(
            r#"{{
                "photos": {{
                    "page": 1,
                    "pages": {pages},
                    "perpage": 500,
                    "total": {total},
                    "photo": [{photo_list}]
                }},
                "stat": "ok"
            }}"#,
            photo_list = photo_array.join(",")
        )
    }

    #[tokio::test]
    async fn fetch_all_photos_single_page() {
        let mock_server = MockServer::start().await;

        let body = make_photos_response(&["1", "2"], 1);
        Mock::given(any())
            .respond_with(ResponseTemplate::new(200).set_body_string(body))
            .mount(&mock_server)
            .await;

        let client = make_client(mock_server.uri());
        let photos = client
            .fetch_all_photos("99@N01", None)
            .await
            .expect("fetch_all_photos");
        assert_eq!(photos.len(), 2);
        assert_eq!(photos[0].id, "1");
        assert_eq!(photos[1].id, "2");
    }

    #[tokio::test]
    async fn fetch_all_photos_truncated_by_max_images() {
        let mock_server = MockServer::start().await;

        let body = make_photos_response(&["1", "2", "3", "4", "5"], 1);
        Mock::given(any())
            .respond_with(ResponseTemplate::new(200).set_body_string(body))
            .mount(&mock_server)
            .await;

        let client = make_client(mock_server.uri());
        let photos = client
            .fetch_all_photos("99@N01", Some(3))
            .await
            .expect("fetch_all_photos with max");
        assert_eq!(photos.len(), 3);
    }

    // ── download_photo ────────────────────────────────────────────────

    #[tokio::test]
    async fn download_photo_writes_file_and_skips_on_second_call() {
        let mock_server = MockServer::start().await;
        let image_bytes = b"FAKE_IMAGE_DATA";

        Mock::given(any())
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_bytes(image_bytes.as_slice())
                    .insert_header("content-type", "image/jpeg"),
            )
            .mount(&mock_server)
            .await;

        let dir = TempDir::new().unwrap();
        let dest = dir.path().join("photo.jpg");

        let photo = Photo {
            id: "999".to_string(),
            owner: "99@N01".to_string(),
            secret: "abc".to_string(),
            server: "1".to_string(),
            farm: 1,
            title: "Test".to_string(),
            ispublic: 1,
            isfriend: 0,
            isfamily: 0,
            datetaken: None,
            originalformat: None,
            url_m: Some(format!("{}/photo.jpg", mock_server.uri())),
            url_l: None,
            url_s: None,
        };

        let client = make_client(mock_server.uri());

        // First call: file should be downloaded
        let downloaded = client
            .download_photo(&photo, &dest)
            .await
            .expect("download_photo first");
        assert!(downloaded, "first download should return true");
        assert!(dest.exists(), "file should exist after download");

        let contents = std::fs::read(&dest).unwrap();
        assert_eq!(contents, image_bytes);

        // Second call: file already exists, should return false
        let downloaded_again = client
            .download_photo(&photo, &dest)
            .await
            .expect("download_photo second");
        assert!(
            !downloaded_again,
            "second download should return false (already exists)"
        );
    }

    #[tokio::test]
    async fn download_photo_with_no_urls_returns_false() {
        let mock_server = MockServer::start().await;
        let dir = TempDir::new().unwrap();
        let dest = dir.path().join("no_url_photo.jpg");

        let photo = Photo {
            id: "000".to_string(),
            owner: "99@N01".to_string(),
            secret: "abc".to_string(),
            server: "1".to_string(),
            farm: 1,
            title: "No URL".to_string(),
            ispublic: 1,
            isfriend: 0,
            isfamily: 0,
            datetaken: None,
            originalformat: None,
            url_m: None,
            url_l: None,
            url_s: None,
        };

        let client = make_client(mock_server.uri());
        let result = client
            .download_photo(&photo, &dest)
            .await
            .expect("download with no URL");
        assert!(!result, "no URL photo should return false");
        assert!(!dest.exists(), "file should not be created");
    }
}
