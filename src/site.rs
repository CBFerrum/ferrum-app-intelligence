//! Intelligence's endpoints, for the signed-in person with their own rights. `POST /api/app/intelligence/ask`
//! `{question}`: an answer, a question back or a plain no, with the plan and the evidence; `GET questions`:
//! the questions they asked; `POST propose` `{metric, why}`: a draft Solution about a metric, for them to
//! finish and someone else to review.

use crate::ask::{ask, propose};
use crate::intent::Rules;
use ferrum_kernel::{Error, Kernel, ListQuery, Result, Site, SiteRequest, SiteResponse};
use serde_json::{Value, json};

pub struct IntelligenceSite;

fn body(req: &SiteRequest) -> Result<Value> {
    if req.body.is_empty() {
        return Ok(json!({}));
    }
    serde_json::from_slice(&req.body).map_err(|e| Error::field("body", format!("is not JSON: {e}")))
}

#[async_trait::async_trait]
impl Site for IntelligenceSite {
    async fn handle(&self, k: &Kernel, req: SiteRequest) -> Result<SiteResponse> {
        let user = req.user.as_ref().ok_or_else(|| Error::Unauthorized("sign in first".into()))?;
        let out = match (req.method.as_str(), req.path.trim_matches('/')) {
            ("POST", "ask") => {
                let b = body(&req)?;
                ask(k, user, &Rules, b["question"].as_str().unwrap_or_default()).await?
            }
            ("GET", "questions") => {
                let q = ListQuery { limit: Some(50), sort: Some("created_at desc".into()), ..Default::default() };
                json!({"questions": k.list(user, "Question", &q).await?.rows})
            }
            ("POST", "propose") => {
                let b = body(&req)?;
                propose(k, user, b["metric"].as_str().unwrap_or_default(), b["why"].as_str().unwrap_or_default()).await?
            }
            _ => return Err(Error::NotFound(format!("intelligence endpoint {} {}", req.method, req.path))),
        };
        let mut r = SiteResponse::json(200, &out);
        r.headers.push(("cache-control".into(), "no-store".into()));
        Ok(r)
    }
}
