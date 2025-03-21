use pipeline::AuthorizationPolicy;
use reqwest::{Client, Url};
use resource_context::ResourceLink;

mod pipeline;
mod resource_context;

fn url_encode(s: impl AsRef<[u8]>) -> String {
    url::form_urlencoded::byte_serialize(s.as_ref()).collect::<String>()
}

pub struct ThinProxyClient {
    endpoint: Url,
    key: String,
    client: Client,
    auth_policy: AuthorizationPolicy,
}

impl ThinProxyClient {
    pub fn new(endpoint: Url, key: String) -> anyhow::Result<Self> {
        let client = Client::builder()
            .user_agent("azure-cosmos-client-engine/thin_proxy")
            .http2_prior_knowledge()
            .build()?;
        let auth_policy = AuthorizationPolicy::from_shared_key(key.clone());
        Ok(Self {
            endpoint,
            key,
            client,
            auth_policy,
        })
    }

    pub async fn send(
        &self,
        link: ResourceLink,
        mut req: reqwest::Request,
    ) -> anyhow::Result<reqwest::Response> {
        self.auth_policy.enrich_request(&link, &mut req).await?;
        let resp = self.client.execute(req).await?;
        Ok(resp)
    }
}
