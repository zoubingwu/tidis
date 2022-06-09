use prometheus::{exponential_buckets, HistogramVec, IntCounter, IntCounterVec, IntGauge};

mod http;

pub use self::http::PrometheusServer;

lazy_static! {
    pub static ref INSTANCE_ID_GAUGER: IntGauge =
        register_int_gauge!("tikv_redis_instance_id", "Instance ID").unwrap();
    pub static ref TIKV_CLIENT_RETRIES: IntGauge =
        register_int_gauge!("tikv_redis_tikv_client_retries", "Client retries").unwrap();
    pub static ref TOTAL_CONNECTION_PROCESSED: IntCounter = register_int_counter!(
        "tikv_redis_total_connection_processed",
        "Total connection processed"
    )
    .unwrap();
    pub static ref DATA_TRAFFIC_OUT: IntCounter =
        register_int_counter!("tikv_redis_data_traffic_out_bytes", "Output data traffic").unwrap();
    pub static ref DATA_TRAFFIC_IN: IntCounter =
        register_int_counter!("tikv_redis_data_traffic_in_bytes", "Input data traffic").unwrap();
    pub static ref REQUEST_COUNTER: IntCounter =
        register_int_counter!("tikv_redis_requests", "Request counter").unwrap();
    pub static ref CURRENT_CONNECTION_COUNTER: IntGauge = register_int_gauge!(
        "tikv_redis_current_connections",
        "Current connection counter"
    )
    .unwrap();
    pub static ref CURRENT_TLS_CONNECTION_COUNTER: IntGauge = register_int_gauge!(
        "tikv_redis_current_tls_connections",
        "Current tls connection counter"
    )
    .unwrap();
    pub static ref REQUEST_CMD_COUNTER: IntCounterVec = register_int_counter_vec!(
        "tikv_redis_command_requests",
        "Request command counter",
        &["cmd"]
    )
    .unwrap();
    pub static ref REQUEST_CMD_FINISH_COUNTER: IntCounterVec = register_int_counter_vec!(
        "tikv_redis_command_requests_finish",
        "Request command finish counter",
        &["cmd"]
    )
    .unwrap();
    pub static ref REQUEST_CMD_HANDLE_TIME: HistogramVec = register_histogram_vec!(
        "tikv_redis_command_handle_time",
        "Bucketed histogram of command handle duration",
        &["cmd"],
        exponential_buckets(0.0005, 2.0, 20).unwrap()
    )
    .unwrap();

    // Trasactions
    pub static ref SNAPSHOT_COUNTER: IntCounter = register_int_counter!("tikv_redis_snapshot_count", "Snapshot count").unwrap();
    pub static ref TXN_COUNTER: IntCounter = register_int_counter!("tikv_redis_txn_count", "Transactions count").unwrap();
    pub static ref TXN_RETRY_COUNTER: IntCounter = register_int_counter!("tikv_redis_txn_retey_count", "Transactions retry count").unwrap();
}
