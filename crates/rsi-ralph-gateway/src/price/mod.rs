//! Price ceilings and provider pins for the models the gateway allows.
//!
//! Every model that may cross the gateway has an entry here. The prices are
//! *ceilings* in US dollars per token: reservations are computed from them,
//! and the upstream is told never to route to a provider charging more
//! (`provider.max_price`). A model without an entry is refused.

use std::collections::BTreeMap;

use crate::error::{Error, Result};

/// GLM 5.3 Flash, the default chat model of a `rsi-ralph` run.
pub const GLM_FLASH: &str = "z-ai/glm-5.3-flash";
/// The default embedding model.
pub const EMBEDDING: &str = "openai/text-embedding-3-small";

/// Price ceilings for one model, and the provider it is pinned to.
#[derive(Debug, Clone, PartialEq)]
pub struct Price {
    /// Dollars per prompt token.
    pub prompt: f64,
    /// Dollars per completion token.
    pub completion: f64,
    /// The only upstream provider the model may be routed to, or
    /// [`ANY_PROVIDER`] to leave routing to the upstream.
    pub provider: String,
}

/// Dollars for `tokens` at `per_token`, saturating at `u32::MAX` tokens.
///
/// Counts above four billion tokens never occur for one call; saturating
/// keeps the conversion exact for every real count.
/// A [`Price::provider`] that pins no provider: the upstream routes the
/// request itself. Use it with an upstream that is a router rather than
/// `OpenRouter` (set with `TINYRSI_UPSTREAM`), which refuses `OpenRouter`'s
/// `provider` routing object. The price ceilings still bound spend.
pub const ANY_PROVIDER: &str = "*";

pub(crate) fn usd(tokens: u64, per_token: f64) -> f64 {
    f64::from(u32::try_from(tokens).unwrap_or(u32::MAX)) * per_token
}

/// The allowlist: model id to price ceiling.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PriceTable(BTreeMap<String, Price>);

impl PriceTable {
    /// An empty table, which allows no model until [`PriceTable::insert`].
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The default table: GLM 5.3 Flash pinned to `z-ai` and the embedding
    /// model pinned to `openai`.
    #[must_use]
    pub fn standard() -> Self {
        let mut table = Self::new();
        table.insert(
            GLM_FLASH,
            Price {
                prompt: 0.30e-6,
                completion: 1.00e-6,
                provider: "z-ai".to_owned(),
            },
        );
        table.insert(
            EMBEDDING,
            Price {
                prompt: 0.02e-6,
                completion: 0.0,
                provider: "openai".to_owned(),
            },
        );
        table
    }

    /// Allows `model` at `price`, replacing any earlier entry.
    pub fn insert(&mut self, model: impl Into<String>, price: Price) {
        self.0.insert(model.into(), price);
    }

    /// The ceiling for `model`.
    ///
    /// # Errors
    ///
    /// Returns [`Error::UnknownModel`] if the model is not allowed.
    pub fn get(&self, model: &str) -> Result<&Price> {
        self.0
            .get(model)
            .ok_or_else(|| Error::UnknownModel(model.to_owned()))
    }

    /// Every allowed model id, in order.
    pub fn models(&self) -> impl Iterator<Item = &str> {
        self.0.keys().map(String::as_str)
    }
}

#[cfg(test)]
mod test;
