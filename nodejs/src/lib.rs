#![deny(clippy::all)]

use std::fmt::Debug;

use azure_cosmoscx::query::QueryClauseItem;
use napi::{JsObject, JsTypedArray};
use tracing_subscriber::EnvFilter;

#[macro_use]
extern crate napi_derive;

#[napi]
pub fn enable_tracing() {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_env("COSMOSCX_LOG"))
        .init();
}

#[napi]
fn version() -> &'static str {
    azure_cosmoscx::version()
}

#[napi]
pub struct QueryPipeline {
    pipeline: azure_cosmoscx::query::QueryPipeline<JsPayload, JsQueryClauseItem>,
}

#[napi]
impl QueryPipeline {
    #[napi(constructor)]
    pub fn new(query: String, plan: JsObject, pkranges: JsObject) {
        todo!()
    }
}

struct JsPayload(JsObject);

impl Debug for JsPayload {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("JsPayload").finish()
    }
}

struct JsQueryClauseItem(JsObject);

impl Debug for JsQueryClauseItem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("JsQueryClauseItem").finish()
    }
}

impl QueryClauseItem for JsQueryClauseItem {
    fn compare(&self, other: &Self) -> Result<std::cmp::Ordering, azure_cosmoscx::Error> {
        todo!()
    }
}
