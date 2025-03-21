// Copyright (c) Microsoft Corporation. All rights reserved.
// Licensed under the MIT License.

//! Defines Cosmos DB's unique Authentication Policy.
//!
//! The Cosmos DB data plane doesn't use a standard `Authorization: Bearer` header for authentication.
//! Instead, it uses a custom header format, as defined in the [official documentation](https://learn.microsoft.com/rest/api/cosmos-db/access-control-on-cosmosdb-resources).
//! We implement that policy here, because we can't use any standard Azure SDK authentication policy.

use reqwest::Request;
use reqwest::header::HeaderValue;
use time::OffsetDateTime;
use tracing::trace;
use url::Url;

use crate::{
    pipeline::signature_target::SignatureTarget, resource_context::ResourceLink, url_encode,
};

const AZURE_VERSION: &str = "2020-07-15";
const MS_DATE: &'static str = "x-ms-date";
const VERSION: &'static str = "x-ms-version";
const AUTHORIZATION: &'static str = "authorization";

#[derive(Debug, Clone)]
enum Credential {
    /// The credential is an Entra ID token.
    Token(String),

    /// The credential is a key to be used to sign the HTTP request (a shared key)
    PrimaryKey(String),
}

#[derive(Debug, Clone)]
pub struct AuthorizationPolicy {
    credential: Credential,
}

impl AuthorizationPolicy {
    pub(crate) fn from_token_credential(token: String) -> Self {
        Self {
            credential: Credential::Token(token),
        }
    }

    pub(crate) fn from_shared_key(key: String) -> Self {
        Self {
            credential: Credential::PrimaryKey(key),
        }
    }
}

impl AuthorizationPolicy {
    pub async fn enrich_request(
        &self,
        resource_link: &ResourceLink,
        request: &mut Request,
    ) -> anyhow::Result<()> {
        trace!("called AuthorizationPolicy::send. self == {:#?}", self);

        // x-ms-date and the string used in the signature must be exactly the same, so just generate it here once.
        let date_string = super::to_rfc7231(&OffsetDateTime::now_utc()).to_lowercase();

        let auth = generate_authorization(
            &self.credential,
            request.url(),
            SignatureTarget::new(request.method().clone(), resource_link, &date_string),
        )
        .await?;

        let headers = request.headers_mut();
        headers.append(MS_DATE, HeaderValue::from_str(&date_string)?);
        headers.append(VERSION, HeaderValue::from_str(AZURE_VERSION)?);
        headers.append(AUTHORIZATION, HeaderValue::from_str(&auth)?);

        Ok(())
    }
}

/// Generates the 'Authorization' header value based on the provided values.
///
/// The specific result format depends on the type of the auth token provided.
///   - "primary": one of the two service-level tokens
///   - "aad": Azure Active Directory token
///
/// In the "primary" case the signature must be constructed by signing the HTTP method,
/// resource type, resource link (the relative URI) and the current time.
///
/// In the "aad" case, the signature is the AAD token.
///
/// NOTE: Resource tokens are not yet supported.
async fn generate_authorization(
    auth_token: &Credential,
    url: &Url,
    signature_target: SignatureTarget<'_>,
) -> anyhow::Result<String> {
    let token = match auth_token {
        Credential::Token(token) => {
            format!("type=aad&ver=1.0&sig={token}")
        }

        Credential::PrimaryKey(key) => signature_target.into_authorization(key)?,
    };

    Ok(url_encode(token))
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use time::OffsetDateTime;
    use url::Url;

    use crate::{
        pipeline::{
            authorization_policy::{Credential, generate_authorization},
            parse_rfc3339,
            signature_target::SignatureTarget,
            to_rfc7231,
        },
        resource_context::{ResourceLink, ResourceType},
    };

    #[tokio::test]
    async fn generate_authorization_for_primary_key_0() {
        let time_nonce = parse_rfc3339("1900-01-01T01:00:00.000000000+00:00").unwrap();
        let date_string = to_rfc7231(&time_nonce).to_lowercase();

        let auth_token = Credential::PrimaryKey(
            "8F8xXXOptJxkblM1DBXW7a6NMI5oE8NnwPGYBmwxLCKfejOK7B7yhcCHMGvN3PBrlMLIOeol1Hv9RCdzAZR5sg==".into(),
        );

        // Use a fake URL since the actual endpoint URL is not important for this test
        let url = Url::parse("https://test_account.example.com/dbs/ToDoList").unwrap();

        let ret = generate_authorization(
            &auth_token,
            &url,
            SignatureTarget::new(
                azure_core::Method::Get,
                &ResourceLink::root(ResourceType::Databases)
                    .item("MyDatabase")
                    .feed(ResourceType::Containers)
                    .item("MyCollection"),
                &date_string,
            ),
        )
        .await
        .unwrap();

        let expected: String =
            url_encode(b"type=master&ver=1.0&sig=vrHmd02almbIg1e4htVWH+Eg/OhEHip3VTwFivZLH0A=");

        assert_eq!(ret, expected);
    }

    #[tokio::test]
    async fn generate_authorization_for_primary_key_1() {
        let time_nonce = parse_rfc3339("2017-04-27T00:51:12.000000000+00:00").unwrap();
        let date_string = to_rfc7231(&time_nonce).to_lowercase();

        let auth_token = Credential::PrimaryKey(
            "dsZQi3KtZmCv1ljt3VNWNm7sQUF1y5rJfC6kv5JiwvW0EndXdDku/dkKBp8/ufDToSxL".into(),
        );

        // Use a fake URL since the actual endpoint URL is not important for this test
        let url = Url::parse("https://test_account.example.com/dbs/ToDoList").unwrap();

        let ret = generate_authorization(
            &auth_token,
            &url,
            SignatureTarget::new(
                reqwest::Method::GET,
                &ResourceLink::root(ResourceType::Databases).item("ToDoList"),
                &date_string,
            ),
        )
        .await
        .unwrap();

        let expected: String =
            url_encode(b"type=master&ver=1.0&sig=KvBM8vONofkv3yKm/8zD9MEGlbu6jjHDJBp4E9c2ZZI=");

        assert_eq!(ret, expected);
    }

    #[test]
    fn scope_from_url_extracts_correct_scope() {
        let scope = scope_from_url(&Url::parse("https://example.com/dbs/test_db/colls").unwrap());
        assert_eq!(scope, "https://example.com/.default");
    }
}
