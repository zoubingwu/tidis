use pprof::protos::Message;
use std::collections::{HashMap, LinkedList};
use std::fs::File;
use std::io::Write;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering::Relaxed;
use std::sync::{Arc, RwLock};
use std::time::Duration;
use tokio::sync::Mutex;

use tikv_client::{RawClient, Transaction, TransactionClient};

use crate::config::LOGGER;
use crate::tikv::encoding::KeyEncoder;
use crate::tikv::errors::REDIS_BACKEND_NOT_CONNECTED_ERR;
use crate::{
    backend_allow_batch_or_default, backend_ca_file_or_default, backend_cert_file_or_default,
    backend_completion_queue_size_or_default, backend_grpc_keepalive_time_or_default,
    backend_grpc_keepalive_timeout_or_default, backend_key_file_or_default,
    backend_max_batch_size_or_default, backend_max_batch_wait_time_or_default,
    backend_max_inflight_requests_or_default, backend_overload_threshold_or_default,
    backend_timeout_or_default, config_meta_key_number_or_default, conn_concurrency_or_default,
    fetch_idx_and_add,
};

use self::client::RawClientWrapper;
use self::client::TxnClientWrapper;

use self::errors::{AsyncResult, RTError};

pub mod client;
pub mod encoding;
pub mod errors;
pub mod hash;
pub mod list;
pub mod lua;
pub mod set;
pub mod string;
pub mod zset;

lazy_static! {
    pub static ref PD_ADDRS: Arc<RwLock<Option<Vec<String>>>> = Arc::new(RwLock::new(None));
    pub static ref TIKV_TRANSACTIONS: Arc<RwLock<HashMap<u64, Transaction>>> =
        Arc::new(RwLock::new(HashMap::new()));
    pub static ref TIKV_TNX_CONN_POOL: Arc<Mutex<LinkedList<TransactionClient>>> =
        Arc::new(Mutex::new(LinkedList::new()));
    pub static ref KEY_ENCODER: KeyEncoder = KeyEncoder::new();
}

pub static mut PROFILER_GUARD: Option<pprof::ProfilerGuard> = None;

pub fn start_profiler() {
    unsafe {
        let guard = pprof::ProfilerGuardBuilder::default()
            .frequency(99)
            //.blocklist(&["libc", "libgcc", "pthread", "vdso"])
            .build()
            .unwrap();
        PROFILER_GUARD = Some(guard);
    }
}

pub fn stop_profiler() {
    unsafe {
        if let Some(guard) = &PROFILER_GUARD {
            if let Ok(report) = guard.report().build() {
                // generate flamegraph file
                let flame_graph_file = File::create("tikv-service-server-flamegraph.svg").unwrap();
                report.flamegraph(flame_graph_file).unwrap();

                // generate profile file
                let mut profile_file = File::create("tikv-service-server-profile.pb").unwrap();
                let profile = report.pprof().unwrap();
                let mut content = Vec::new();
                profile.write_to_vec(&mut content).unwrap();
                profile_file.write_all(&content).unwrap();
            };
            PROFILER_GUARD.take();
        }
    }
}

pub static mut TIKV_RAW_CLIENT: Option<RawClient> = None;

pub static mut TIKV_TXN_CLIENTS: Option<Vec<TransactionClient>> = None;
pub static mut TIKV_TXN_CLIENT_IDX: AtomicUsize = AtomicUsize::new(0);

pub static mut INSTANCE_ID: u64 = 0;

pub fn set_instance_id(id: u64) {
    unsafe {
        INSTANCE_ID = id;
    }
}

pub fn get_instance_id() -> u64 {
    unsafe { INSTANCE_ID }
}

pub fn get_client() -> Result<RawClientWrapper, RTError> {
    if unsafe { TIKV_RAW_CLIENT.is_none() } {
        return Err(REDIS_BACKEND_NOT_CONNECTED_ERR);
    }
    let client = unsafe { TIKV_RAW_CLIENT.as_ref().unwrap() };
    let ret = RawClientWrapper::new(client);
    Ok(ret)
}

pub fn get_txn_client() -> Result<TxnClientWrapper<'static>, RTError> {
    if unsafe { TIKV_RAW_CLIENT.is_none() } {
        return Err(REDIS_BACKEND_NOT_CONNECTED_ERR);
    }
    let client = unsafe {
        let mut idx = TIKV_TXN_CLIENT_IDX.load(Relaxed);
        idx = (idx + 1) % TIKV_TXN_CLIENTS.as_ref().unwrap().len();
        TIKV_TXN_CLIENT_IDX.store(idx, Relaxed);

        &TIKV_TXN_CLIENTS.as_ref().unwrap()[idx]
    };
    let ret = TxnClientWrapper::new(client);
    Ok(ret)
}

pub async fn sleep(ms: u32) {
    tokio::time::sleep(Duration::from_millis(ms as u64)).await;
}

pub async fn do_async_txn_connect(addrs: Vec<String>) -> AsyncResult<()> {
    PD_ADDRS.write().unwrap().replace(addrs.clone());

    let mut config = tikv_client::Config::default()
        .with_timeout(Duration::from_millis(backend_timeout_or_default()))
        .with_kv_timeout(backend_timeout_or_default())
        .with_kv_allow_batch(backend_allow_batch_or_default())
        .with_kv_completion_queue_size(backend_completion_queue_size_or_default())
        .with_kv_grpc_keepalive_time(backend_grpc_keepalive_time_or_default())
        .with_kv_grpc_keepalive_timeout(backend_grpc_keepalive_timeout_or_default())
        .with_kv_allow_batch(backend_allow_batch_or_default())
        .with_kv_overload_threshold(backend_overload_threshold_or_default())
        .with_kv_max_batch_size(backend_max_batch_size_or_default())
        .with_kv_max_inflight_requests(backend_max_inflight_requests_or_default())
        .with_kv_max_batch_wait_time(backend_max_batch_wait_time_or_default());
    if !backend_ca_file_or_default().is_empty()
        || !backend_cert_file_or_default().is_empty()
        || !backend_key_file_or_default().is_empty()
    {
        config = config.with_security(
            backend_ca_file_or_default(),
            backend_cert_file_or_default(),
            backend_key_file_or_default(),
        );
    }
    let mut clients = Vec::with_capacity(conn_concurrency_or_default());
    for _ in 0..conn_concurrency_or_default() {
        let client =
            TransactionClient::new_with_config(addrs.clone(), config.clone(), Some(LOGGER.clone()))
                .await?;
        clients.push(client);
    }
    unsafe {
        TIKV_TXN_CLIENTS.replace(clients);
    }

    Ok(())
}

pub async fn do_async_raw_connect(addrs: Vec<String>) -> AsyncResult<()> {
    let mut config = tikv_client::Config::default()
        .with_timeout(Duration::from_millis(backend_timeout_or_default()));
    if !backend_ca_file_or_default().is_empty()
        || !backend_cert_file_or_default().is_empty()
        || !backend_key_file_or_default().is_empty()
    {
        config = config.with_security(
            backend_ca_file_or_default(),
            backend_cert_file_or_default(),
            backend_key_file_or_default(),
        );
    }
    let client = RawClient::new_with_config(addrs.clone(), config, Some(LOGGER.clone())).await?;
    unsafe {
        TIKV_RAW_CLIENT.replace(client);
    }
    Ok(())
}

pub async fn do_async_connect(addrs: Vec<String>) -> AsyncResult<()> {
    do_async_txn_connect(addrs.clone()).await?;
    do_async_raw_connect(addrs).await?;
    Ok(())
}

pub fn gen_next_meta_index() -> u16 {
    fetch_idx_and_add() % config_meta_key_number_or_default()
}
