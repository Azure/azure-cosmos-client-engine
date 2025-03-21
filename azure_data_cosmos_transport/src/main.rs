use azure_data_cosmos_transport::ThinProxyClient;

fn main() {
    let endpoint = std::env::var("COSMOS_ENDPOINT").expect("AZURE_COSMOS_ENDPOINT must be set");
    let key = std::env::var("COSMOS_KEY").expect("AZURE_COSMOS_KEY must be set");
    let client = ThinProxyClient::new(endpoint.parse().unwrap(), key).unwrap();
}
