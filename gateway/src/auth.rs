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
    /// 방의 회차. 같은 ID로 방을 다시 열 때마다 커진다 (관리 API가 방을 만든 시각을 넣는다).
    /// 닫힌 회차나 지난 회차의 토큰으로 들어오는 것을 막는 데 쓴다.
    /// 개발 도구(loadgen, 테스트)가 직접 서명한 토큰에는 없을 수 있다.
    #[serde(default, rename = "gen")]
    pub generation: Option<u64>,
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
