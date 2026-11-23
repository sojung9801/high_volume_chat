use anyhow::Result;
use jsonwebtoken::{Algorithm, DecodingKey, Validation, decode};
use serde::Deserialize;

/// 관리 API가 발급하는 입장 토큰의 audience. apps/api 쪽 상수와 같아야 한다.
const JOIN_TOKEN_AUDIENCE: &str = "chat-gateway";

#[derive(Debug, Deserialize)]
pub struct JoinClaims {
    /// 사용자 ID.
    pub sub: String,
    pub room: String,
    pub nick: String,
    /// 방장인지. 관리 API가 방장 열쇠를 확인한 경우에만 true로 싣는다.
    #[serde(default)]
    pub owner: bool,
}

pub struct JoinTokenVerifier {
    key: DecodingKey,
    validation: Validation,
}

impl JoinTokenVerifier {
    pub fn new(secret: &str) -> Result<Self> {
        let mut validation = Validation::new(Algorithm::HS256);
        validation.set_audience(&[JOIN_TOKEN_AUDIENCE]);
        validation.set_required_spec_claims(&["exp", "aud", "sub"]);

        Ok(Self {
            key: DecodingKey::from_secret(secret.as_bytes())?,
            validation,
        })
    }

    pub fn verify(&self, token: &str) -> Result<JoinClaims, jsonwebtoken::errors::Error> {
        decode::<JoinClaims>(token, &self.key, &self.validation).map(|data| data.claims)
    }
}
