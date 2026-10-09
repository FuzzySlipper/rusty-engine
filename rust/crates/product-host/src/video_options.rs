//! The video-options route: `GET` and `POST /__rusty/product/runtime/video-options`.
//!
//! The Engine's video options panel (`@rusty-engine/video-options`) reads the
//! renderer settings catalogue with the values in effect, and changes the
//! player's choices, here. This is presentation, not gameplay input: the
//! runtime applies a choice to the renderer at once and stores it for the
//! install, without a product call. The host only carries the requests; the
//! runtime that draws answers them.

use std::sync::{Arc, Mutex};

pub const PRODUCT_HOST_VIDEO_OPTIONS_PATH: &str = "/__rusty/product/runtime/video-options";

/// What the page asks.
#[derive(Debug, Clone, PartialEq)]
pub enum ProductHostVideoOptionsRequest {
    /// The catalogue with the values in effect.
    Read,
    /// A change of the player's choices, as the page sent it (JSON).
    Change(serde_json::Value),
}

type Handler =
    Box<dyn Fn(ProductHostVideoOptionsRequest) -> Result<serde_json::Value, String> + Send + Sync>;

/// The runtime's answer to video-options requests, shared with the host.
#[derive(Default)]
pub struct ProductHostVideoOptions {
    handler: Mutex<Option<Handler>>,
}

impl ProductHostVideoOptions {
    pub fn new() -> Arc<Self> {
        Arc::default()
    }

    /// Answers every request from now on: the catalogue JSON, or why a
    /// change is refused.
    pub fn set_handler(
        &self,
        handler: impl Fn(ProductHostVideoOptionsRequest) -> Result<serde_json::Value, String>
            + Send
            + Sync
            + 'static,
    ) {
        *self
            .handler
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(Box::new(handler));
    }

    /// `None` when no runtime answers (one that draws nothing).
    pub(crate) fn answer(
        &self,
        request: ProductHostVideoOptionsRequest,
    ) -> Option<Result<serde_json::Value, String>> {
        self.handler
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .as_ref()
            .map(|handler| handler(request))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_handler_answers_nothing_and_a_handler_answers_each_request() {
        let options = ProductHostVideoOptions::new();
        assert!(options
            .answer(ProductHostVideoOptionsRequest::Read)
            .is_none());
        options.set_handler(|request| match request {
            ProductHostVideoOptionsRequest::Read => Ok(serde_json::json!({"options": []})),
            ProductHostVideoOptionsRequest::Change(body) => Err(format!("refused {body}")),
        });
        assert_eq!(
            options.answer(ProductHostVideoOptionsRequest::Read),
            Some(Ok(serde_json::json!({"options": []})))
        );
        assert_eq!(
            options.answer(ProductHostVideoOptionsRequest::Change(serde_json::json!(1))),
            Some(Err("refused 1".to_owned()))
        );
    }
}
