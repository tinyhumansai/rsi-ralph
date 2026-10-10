//! Tests for the `TinyBus` module adapter and its declared surface.

use std::sync::Mutex;

use super::{LineageService, setup};
use tinybus::broker::Broker;
use tinybus::transport::memory::MemoryBus;
use tinybus::{Connection, Interface};
use tinyrsi_bus::{
    GradeEntry, LineageAppendRequest, LineageAppendResponse, LineageHeadRequest,
    LineageHeadResponse, Verdict, names,
};

#[test]
fn declared_methods_match_the_dispatch_table() {
    let methods = LineageService {
        lineage: Mutex::new(crate::Lineage::new()),
    }
    .members()
    .into_iter()
    .map(|member| member.to_string())
    .collect::<Vec<_>>();

    assert_eq!(methods, names::METHODS.to_vec());
}

#[test]
fn the_served_interface_name_matches_the_contract() {
    assert_eq!(
        LineageService {
            lineage: Mutex::new(crate::Lineage::new()),
        }
        .name()
        .to_string(),
        names::INTERFACE
    );
}

fn baseline_entry() -> GradeEntry {
    GradeEntry::new(0, "sha256:abc", 0.703, Verdict::Accepted, "baseline")
}

#[tokio::test]
async fn module_records_and_reads_entries_over_a_real_bus() -> tinybus::Result<()> {
    let bus = MemoryBus::new();
    Broker::new().spawn(bus.clone());

    let service = Connection::connect(bus.connect().await?).await?;
    setup(service.clone()).await?;

    let client = Connection::connect(bus.connect().await?).await?;
    let proxy = client.proxy(names::INTERFACE, names::OBJECT_PATH, names::INTERFACE)?;

    let empty: LineageHeadResponse = proxy
        .call(names::methods::HEAD, (LineageHeadRequest::new(),))
        .await?;
    assert!(empty.entry.is_none());

    let recorded: LineageAppendResponse = proxy
        .call(
            names::methods::APPEND,
            (LineageAppendRequest::new(baseline_entry()),),
        )
        .await?;
    assert!(recorded.recorded);

    let head: LineageHeadResponse = proxy
        .call(names::methods::HEAD, (LineageHeadRequest::new(),))
        .await?;
    assert_eq!(head.entry.as_ref().map(|e| e.step), Some(0));
    Ok(())
}

#[tokio::test]
async fn module_rejects_an_out_of_order_step_over_the_bus() -> tinybus::Result<()> {
    let bus = MemoryBus::new();
    Broker::new().spawn(bus.clone());

    let service = Connection::connect(bus.connect().await?).await?;
    setup(service.clone()).await?;

    let client = Connection::connect(bus.connect().await?).await?;
    let proxy = client.proxy(names::INTERFACE, names::OBJECT_PATH, names::INTERFACE)?;
    let skipped = GradeEntry::new(2, "sha256:abc", 0.5, Verdict::Accepted, "rationale");
    let result = proxy
        .call::<LineageAppendResponse>(
            names::methods::APPEND,
            (LineageAppendRequest::new(skipped),),
        )
        .await;

    let Err(error) = result else {
        return Err(tinybus::Error::failed(
            "an out-of-order step unexpectedly succeeded",
        ));
    };
    assert!(error.to_string().contains("does not continue the ledger"));
    Ok(())
}
