use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProfileCookieJarRecord {
    pub profile_id: String,
    pub cookies_json: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProfileCredentialRecord {
    pub id: String,
    pub profile_id: String,
    pub platform: String,
    pub username: String,
    pub password: String,
    pub notes: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone)]
pub struct CreateProfileCredential {
    pub profile_id: String,
    pub platform: String,
    pub username: String,
    pub password: String,
    pub notes: String,
}

#[derive(Debug, Clone)]
pub struct UpdateProfileCredential {
    pub platform: String,
    pub username: String,
    pub password: String,
    pub notes: String,
}
