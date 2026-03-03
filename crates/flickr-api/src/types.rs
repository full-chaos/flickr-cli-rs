use serde::{Deserialize, Serialize};

/// OAuth 1.0a tokens stored after authentication.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OAuthTokens {
    pub oauth_token: String,
    pub oauth_token_secret: String,
    #[serde(default)]
    pub fullname: Option<String>,
    #[serde(default)]
    pub user_nsid: Option<String>,
    #[serde(default)]
    pub username: Option<String>,
}

/// A single Flickr photo returned from the API.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Photo {
    pub id: String,
    pub owner: String,
    pub secret: String,
    pub server: String,
    pub farm: i64,
    pub title: String,
    #[serde(default)]
    pub ispublic: i32,
    #[serde(default)]
    pub isfriend: i32,
    #[serde(default)]
    pub isfamily: i32,
    #[serde(default)]
    pub datetaken: Option<String>,
    #[serde(default)]
    pub originalformat: Option<String>,
    #[serde(default)]
    pub url_m: Option<String>,
    #[serde(default)]
    pub url_l: Option<String>,
    #[serde(default)]
    pub url_s: Option<String>,
}

impl Photo {
    /// Returns the best available image URL (medium > large > small).
    pub fn best_url(&self) -> Option<&str> {
        self.url_m
            .as_deref()
            .or(self.url_l.as_deref())
            .or(self.url_s.as_deref())
    }
}

/// Paginated photos response from flickr.people.getPhotos.
#[derive(Debug, Deserialize)]
pub struct PhotosPage {
    pub page: u32,
    pub pages: u32,
    pub perpage: u32,
    pub total: u64,
    pub photo: Vec<Photo>,
}

#[derive(Debug, Deserialize)]
pub struct PhotosResponse {
    pub photos: PhotosPage,
    pub stat: String,
}

/// Response from flickr.test.login.
#[derive(Debug, Deserialize)]
pub struct LoginResponse {
    pub user: UserData,
    pub stat: String,
}

#[derive(Debug, Deserialize)]
pub struct UserData {
    pub id: String,
    pub username: Option<UsernameContent>,
}

#[derive(Debug, Deserialize)]
pub struct UsernameContent {
    #[serde(rename = "_content")]
    pub content: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_photo() -> Photo {
        Photo {
            id: "123456".to_string(),
            owner: "99@N01".to_string(),
            secret: "abc123".to_string(),
            server: "65535".to_string(),
            farm: 1,
            title: "Test Photo".to_string(),
            ispublic: 1,
            isfriend: 0,
            isfamily: 0,
            datetaken: None,
            originalformat: None,
            url_m: None,
            url_l: None,
            url_s: None,
        }
    }

    // ── Photo::best_url ───────────────────────────────────────────────

    #[test]
    fn best_url_prefers_url_m() {
        let photo = Photo {
            url_m: Some("https://example.com/m.jpg".to_string()),
            url_l: Some("https://example.com/l.jpg".to_string()),
            url_s: Some("https://example.com/s.jpg".to_string()),
            ..make_photo()
        };
        assert_eq!(photo.best_url(), Some("https://example.com/m.jpg"));
    }

    #[test]
    fn best_url_falls_back_to_url_l() {
        let photo = Photo {
            url_m: None,
            url_l: Some("https://example.com/l.jpg".to_string()),
            url_s: Some("https://example.com/s.jpg".to_string()),
            ..make_photo()
        };
        assert_eq!(photo.best_url(), Some("https://example.com/l.jpg"));
    }

    #[test]
    fn best_url_falls_back_to_url_s() {
        let photo = Photo {
            url_m: None,
            url_l: None,
            url_s: Some("https://example.com/s.jpg".to_string()),
            ..make_photo()
        };
        assert_eq!(photo.best_url(), Some("https://example.com/s.jpg"));
    }

    #[test]
    fn best_url_returns_none_when_all_missing() {
        let photo = Photo {
            url_m: None,
            url_l: None,
            url_s: None,
            ..make_photo()
        };
        assert_eq!(photo.best_url(), None);
    }

    // ── OAuthTokens serde ─────────────────────────────────────────────

    #[test]
    fn oauth_tokens_round_trip() {
        let tokens = OAuthTokens {
            oauth_token: "token123".to_string(),
            oauth_token_secret: "secret456".to_string(),
            fullname: Some("Test User".to_string()),
            user_nsid: Some("99@N01".to_string()),
            username: Some("testuser".to_string()),
        };
        let json = serde_json::to_string(&tokens).expect("serialize");
        let restored: OAuthTokens = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(restored.oauth_token, tokens.oauth_token);
        assert_eq!(restored.oauth_token_secret, tokens.oauth_token_secret);
        assert_eq!(restored.fullname, tokens.fullname);
        assert_eq!(restored.user_nsid, tokens.user_nsid);
        assert_eq!(restored.username, tokens.username);
    }

    #[test]
    fn oauth_tokens_missing_optional_fields_default_to_none() {
        let json = r#"{"oauth_token":"tok","oauth_token_secret":"sec"}"#;
        let tokens: OAuthTokens = serde_json::from_str(json).expect("deserialize");
        assert_eq!(tokens.oauth_token, "tok");
        assert_eq!(tokens.oauth_token_secret, "sec");
        assert_eq!(tokens.fullname, None);
        assert_eq!(tokens.user_nsid, None);
        assert_eq!(tokens.username, None);
    }

    // ── Photo deserialization ─────────────────────────────────────────

    #[test]
    fn photo_deserializes_from_flickr_json() {
        let json = r#"{
            "id": "54321",
            "owner": "12345@N01",
            "secret": "abcdef",
            "server": "65535",
            "farm": 66,
            "title": "Sunset",
            "ispublic": 1,
            "isfriend": 0,
            "isfamily": 0,
            "datetaken": "2024-01-15 10:30:00",
            "originalformat": "jpg",
            "url_m": "https://live.staticflickr.com/65535/54321_abcdef_m.jpg",
            "url_l": "https://live.staticflickr.com/65535/54321_abcdef_b.jpg",
            "url_s": "https://live.staticflickr.com/65535/54321_abcdef_s.jpg"
        }"#;
        let photo: Photo = serde_json::from_str(json).expect("deserialize photo");
        assert_eq!(photo.id, "54321");
        assert_eq!(photo.owner, "12345@N01");
        assert_eq!(photo.farm, 66);
        assert_eq!(photo.title, "Sunset");
        assert_eq!(photo.datetaken.as_deref(), Some("2024-01-15 10:30:00"));
        assert_eq!(photo.originalformat.as_deref(), Some("jpg"));
        assert!(photo.url_m.is_some());
    }

    // ── PhotosResponse deserialization ────────────────────────────────

    #[test]
    fn photos_response_deserializes_paginated() {
        let json = r#"{
            "photos": {
                "page": 1,
                "pages": 3,
                "perpage": 500,
                "total": 1234,
                "photo": [
                    {
                        "id": "111",
                        "owner": "99@N01",
                        "secret": "s1",
                        "server": "1",
                        "farm": 1,
                        "title": "Photo A",
                        "ispublic": 1,
                        "isfriend": 0,
                        "isfamily": 0
                    },
                    {
                        "id": "222",
                        "owner": "99@N01",
                        "secret": "s2",
                        "server": "2",
                        "farm": 2,
                        "title": "Photo B",
                        "ispublic": 0,
                        "isfriend": 1,
                        "isfamily": 0
                    }
                ]
            },
            "stat": "ok"
        }"#;
        let resp: PhotosResponse = serde_json::from_str(json).expect("deserialize");
        assert_eq!(resp.stat, "ok");
        assert_eq!(resp.photos.page, 1);
        assert_eq!(resp.photos.pages, 3);
        assert_eq!(resp.photos.perpage, 500);
        assert_eq!(resp.photos.total, 1234);
        assert_eq!(resp.photos.photo.len(), 2);
        assert_eq!(resp.photos.photo[0].id, "111");
        assert_eq!(resp.photos.photo[1].id, "222");
    }

    // ── LoginResponse deserialization ─────────────────────────────────

    #[test]
    fn login_response_deserializes_with_content_rename() {
        let json = r#"{
            "user": {
                "id": "12345@N01",
                "username": {
                    "_content": "testuser"
                }
            },
            "stat": "ok"
        }"#;
        let resp: LoginResponse = serde_json::from_str(json).expect("deserialize");
        assert_eq!(resp.stat, "ok");
        assert_eq!(resp.user.id, "12345@N01");
        let username = resp.user.username.expect("username present");
        assert_eq!(username.content, "testuser");
    }

    #[test]
    fn login_response_deserializes_without_username() {
        let json = r#"{
            "user": {
                "id": "12345@N01"
            },
            "stat": "ok"
        }"#;
        let resp: LoginResponse = serde_json::from_str(json).expect("deserialize");
        assert_eq!(resp.user.id, "12345@N01");
        assert!(resp.user.username.is_none());
    }
}
