use super::*;
use crate::service::test_helpers::TestContext;
use fuel_core_interfaces::txpool::{
    Error as TxpoolError,
    TxPoolMpsc,
    TxStatus,
    TxStatusBroadcast,
};
use tokio::sync::oneshot;

#[tokio::test]
async fn test_start_stop() {
    let ctx = TestContext::new().await;

    let service = ctx.service();

    // Double start will return false.
    assert!(service.start().await.is_err(), "double start should fail");

    let stop_handle = service.stop().await;
    assert!(stop_handle.is_some());
    let _ = stop_handle.unwrap().await;

    assert!(service.start().await.is_ok(), "Should start again");
}

#[tokio::test]
async fn test_filter_by_negative() {
    let ctx = TestContext::new().await;

    let tx1 = Arc::new(ctx.setup_script_tx(10));
    let tx2 = Arc::new(ctx.setup_script_tx(20));
    let tx3 = Arc::new(ctx.setup_script_tx(30));

    let service = ctx.service();

    let (response, receiver) = oneshot::channel();
    let _ = service
        .sender()
        .send(TxPoolMpsc::Insert {
            txs: vec![tx1.clone(), tx2.clone()],
            response,
        })
        .await;
    let out = receiver.await.unwrap();

    assert_eq!(out.len(), 2, "Should be len 2:{:?}", out);
    assert!(out[0].is_ok(), "Tx1 should be OK, got err:{:?}", out);
    assert!(out[1].is_ok(), "Tx2 should be OK, got err:{:?}", out);

    let (response, receiver) = oneshot::channel();
    let _ = service
        .sender()
        .send(TxPoolMpsc::FilterByNegative {
            ids: vec![tx1.id(), tx2.id(), tx3.id()],
            response,
        })
        .await;
    let out = receiver.await.unwrap();

    assert_eq!(out.len(), 1, "Should be len 1:{:?}", out);
    assert_eq!(out[0], tx3.id(), "Found tx id match{:?}", out);
    service.stop().await.unwrap().await.unwrap();
}

#[tokio::test]
async fn test_find() {
    let ctx = TestContext::new().await;

    let tx1 = Arc::new(ctx.setup_script_tx(10));
    let tx2 = Arc::new(ctx.setup_script_tx(20));
    let tx3 = Arc::new(ctx.setup_script_tx(30));

    let service = ctx.service();

    let (response, receiver) = oneshot::channel();
    let _ = service
        .sender()
        .send(TxPoolMpsc::Insert {
            txs: vec![tx1.clone(), tx2.clone()],
            response,
        })
        .await;
    let out = receiver.await.unwrap();

    assert_eq!(out.len(), 2, "Should be len 2:{:?}", out);
    assert!(out[0].is_ok(), "Tx1 should be OK, got err:{:?}", out);
    assert!(out[1].is_ok(), "Tx2 should be OK, got err:{:?}", out);
    let (response, receiver) = oneshot::channel();
    let _ = service
        .sender()
        .send(TxPoolMpsc::Find {
            ids: vec![tx1.id(), tx3.id()],
            response,
        })
        .await;
    let out = receiver.await.unwrap();
    assert_eq!(out.len(), 2, "Should be len 2:{:?}", out);
    assert!(out[0].is_some(), "Tx1 should be some:{:?}", out);
    let id = out[0].as_ref().unwrap().id();
    assert_eq!(id, tx1.id(), "Found tx id match{:?}", out);
    assert!(out[1].is_none(), "Tx3 should not be found:{:?}", out);
    service.stop().await.unwrap().await.unwrap();
}

#[tokio::test]
async fn simple_insert_removal_subscription() {
    let ctx = TestContext::new().await;

    let tx1 = Arc::new(ctx.setup_script_tx(10));
    let tx2 = Arc::new(ctx.setup_script_tx(20));
    let service = ctx.service();

    let mut subscribe = service.subscribe_ch();

    let (response, receiver) = oneshot::channel();
    let _ = service
        .sender()
        .send(TxPoolMpsc::Insert {
            txs: vec![tx1.clone(), tx2.clone()],
            response,
        })
        .await;
    let out = receiver.await.unwrap();

    assert!(out[0].is_ok(), "Tx1 should be OK, got err:{:?}", out);
    assert!(out[1].is_ok(), "Tx2 should be OK, got err:{:?}", out);

    // we are sure that included tx are already broadcasted.
    assert_eq!(
        subscribe.try_recv(),
        Ok(TxStatusBroadcast {
            tx: tx1.clone(),
            status: TxStatus::Submitted,
        }),
        "First added should be tx1"
    );
    assert_eq!(
        subscribe.try_recv(),
        Ok(TxStatusBroadcast {
            tx: tx2.clone(),
            status: TxStatus::Submitted,
        }),
        "Second added should be tx2"
    );

    // remove them
    let (response, receiver) = oneshot::channel();
    let _ = service
        .sender()
        .send(TxPoolMpsc::Remove {
            ids: vec![tx1.id(), tx2.id()],
            response,
        })
        .await;
    let _rem = receiver.await.unwrap();

    assert_eq!(
        tokio::time::timeout(std::time::Duration::from_secs(2), subscribe.recv()).await,
        Ok(Ok(TxStatusBroadcast {
            tx: tx1,
            status: TxStatus::SqueezedOut {
                reason: TxpoolError::Removed
            }
        })),
        "First removed should be tx1"
    );

    assert_eq!(
        tokio::time::timeout(std::time::Duration::from_secs(2), subscribe.recv()).await,
        Ok(Ok(TxStatusBroadcast {
            tx: tx2,
            status: TxStatus::SqueezedOut {
                reason: TxpoolError::Removed
            }
        })),
        "Second removed should be tx2"
    );

    service.stop().await.unwrap().await.unwrap();
}