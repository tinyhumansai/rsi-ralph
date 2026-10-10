//! `TinyBus` module entrypoint and bus-facing interface.
//!
//! This adapter keeps the feature implementation independent from `TinyBus`
//! while exposing it as an installable, dynamically loaded integration. The
//! names and payload types it serves come from [`tinyrsi_bus`], so a host
//! spells them from the contract crate instead of repeating string literals.

use std::sync::Mutex;

use tinybus::{Connection, Result as TinyBusResult};
use tinyrsi_bus::{
    LineageAppendRequest, LineageAppendResponse, LineageHeadRequest, LineageHeadResponse, names,
};

struct LineageService {
    lineage: Mutex<crate::Lineage>,
}

#[tinybus::interface(name = "ai.tinyhumans.tinyrsi.Lineage")]
impl LineageService {
    async fn append(&self, request: LineageAppendRequest) -> TinyBusResult<LineageAppendResponse> {
        let recorded = {
            let mut lineage = self
                .lineage
                .lock()
                .map_err(|error| tinybus::Error::failed(error.to_string()))?;
            crate::append(&mut lineage, request)
                .map_err(|error| tinybus::Error::failed(error.to_string()))?
        };
        std::future::ready(Ok(LineageAppendResponse::new(recorded))).await
    }

    async fn head(&self, _request: LineageHeadRequest) -> TinyBusResult<LineageHeadResponse> {
        let head = {
            let lineage = self
                .lineage
                .lock()
                .map_err(|error| tinybus::Error::failed(error.to_string()))?;
            crate::head(&lineage)
        };
        std::future::ready(Ok(head)).await
    }
}

async fn setup(connection: Connection) -> TinyBusResult<()> {
    let service = LineageService {
        lineage: Mutex::new(crate::Lineage::new()),
    };
    connection
        .serve_at(names::OBJECT_PATH.try_into()?, service)
        .await?;
    connection.request_name(names::INTERFACE).await?;
    Ok(())
}

tinybus_module::module_export! {
    setup = setup,
    worker_threads = 1,
    provides = ["ai.tinyhumans.tinyrsi.Lineage"],
    methods = ["Append", "Head"],
    signals = [],
    requires = [],
    optional = [],
    lazy = false,
}

#[cfg(test)]
mod test;
