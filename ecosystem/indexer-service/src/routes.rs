use anyhow::Result;
use aptos_api::{
    page::Page,
    param::{Param, TransactionIdParam},
};
use aptos_api_types::{
    BlockMetadataTransaction as APIBlockMetadataTransaction, Event as APIEvent,
    GenesisTransaction as APIGenesisTransaction, MoveType, Transaction as APITransaction,
    TransactionId, TransactionInfo as APITransactionInfo, UserTransaction as APIUserTransaction,
    UserTransactionRequest as APIUserTransactionRequest, U64,
};
use aptos_indexer::models::events::EventModel;
use aptos_indexer::{database::PgDbPool, models::transactions::TransactionModel};
use futures::FutureExt;
use warp::{any, filters::BoxedFilter, reject, Filter, Rejection, Reply};

/*
// GET /transactions?start={u64}&limit={u16}
pub fn get_transactions(context: Context) -> BoxedFilter<(impl Reply,)> {
    warp::path!("transactions")
        .and(warp::get())
        .and(warp::query::<Page>())
        .and(context.filter())
        .and_then(handle_get_transactions)
        .with(metrics("get_transactions"))
        .boxed()
}
*/

/// GET /transactions/{txn-hash / version}
pub fn get_transaction(context: Context) -> BoxedFilter<(impl Reply,)> {
    warp::path!("transactions" / TransactionIdParam)
        .and(warp::get())
        .and(context.filter())
        .and_then(handle_get_transaction)
        .with(metrics("get_transaction"))
        .boxed()
}

use warp::http::header::{HeaderValue, CONTENT_TYPE};

pub struct Response {
    pub body: Vec<u8>,
    pub err: Option<ServiceError>,
}

impl Response {
    pub fn success<T: serde::Serialize>(body: &T) -> Result<Self> {
        Ok(Self {
            body: serde_json::to_vec(body)?,
            err: None,
        })
    }
    pub fn error(error: ServiceError) -> Self {
        Self {
            body: error.to_string().into_bytes(),
            err: Some(error),
        }
    }
}

impl Reply for Response {
    fn into_response(self) -> warp::reply::Response {
        let mut res = warp::reply::Response::new(self.body.into());
        let headers = res.headers_mut();
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        res
    }
}

#[derive(Debug)]
pub enum ServiceError {
    PoolError(r2d2::Error),
    DatabaseError(diesel::result::Error),
    RequestError(aptos_api_types::Error),
}

impl reject::Reject for ServiceError {}
impl std::error::Error for ServiceError {}

impl std::fmt::Display for ServiceError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            ServiceError::PoolError(e) => write!(f, "ServiceError: {:?}", e),
            ServiceError::DatabaseError(e) => write!(f, "ServiceError: {:?}", e),
            ServiceError::RequestError(e) => write!(f, "ServiceError: {:?}", e),
        }
    }
}

async fn handle_get_transaction(id: TransactionIdParam, db_pool: PgDbPool) -> Response {
    match handle_get_transaction_inner(id, db_pool).await {
        Ok(tx) => Response::success(&tx).unwrap(),
        Err(e) => Response::error(e),
    }
}

async fn handle_get_transaction_inner(
    id: TransactionIdParam,
    db_pool: PgDbPool,
) -> Result<APITransaction, ServiceError> {
    // fail_point("endpoint_get_transaction")?;
    let tx_id = id
        .parse("transaction hash or version")
        .map_err(|e| ServiceError::RequestError(e))?;
    let conn = db_pool.get().map_err(|e| ServiceError::PoolError(e))?;
    let (tx, maybe_ut, maybe_bmt, events) = match tx_id {
        TransactionId::Hash(hash) => TransactionModel::get_by_hash(&hash.to_string(), &conn),
        TransactionId::Version(version) => TransactionModel::get_by_version(version, &conn),
    }
    .map_err(|e| ServiceError::DatabaseError(e))?;

    let events = events
        .into_iter()
        .map(|e: EventModel| APIEvent {
            key: e.key.parse().unwrap(),
            sequence_number: U64(e.sequence_number as u64),
            typ: e.type_.parse().unwrap(),
            data: e.data,
        })
        .collect();

    let tx_info = APITransactionInfo {
        version: U64(tx.version as u64),
        hash: tx.hash.parse().unwrap(),
        state_root_hash: tx.state_root_hash.parse().unwrap(),
        event_root_hash: tx.event_root_hash.parse().unwrap(),
        gas_used: U64(tx.gas_used as u64),
        success: tx.success,
        vm_status: tx.vm_status,
        accumulator_root_hash: tx.accumulator_root_hash.parse().unwrap(),
    };

    let transaction: APITransaction = match tx.type_.as_str() {
        "genesis_transaction" => APITransaction::GenesisTransaction(APIGenesisTransaction {
            info: tx_info,
            payload: serde_json::from_value(tx.payload).unwrap(),
            events,
        }),
        "block_metadata_transaction" => {
            let bmt =
                maybe_bmt.expect("BlockMetadataTransaction was not fetched: database is corrupt!");
            APITransaction::BlockMetadataTransaction(APIBlockMetadataTransaction {
                info: tx_info,
                id: bmt.id.parse().unwrap(),
                round: U64(bmt.round as u64),
                previous_block_votes: serde_json::from_value(bmt.previous_block_votes).unwrap(),
                proposer: bmt.proposer.parse().unwrap(),
                timestamp: U64(bmt.timestamp.timestamp_millis() as u64),
            })
        }
        "user_transaction" => {
            let ut = maybe_ut.expect("UserTransaction was not fetched: database is corrupt!");
            APITransaction::UserTransaction(Box::from(APIUserTransaction {
                info: tx_info,
                request: APIUserTransactionRequest {
                    sender: ut.sender.parse().unwrap(),
                    sequence_number: U64(ut.sequence_number as u64),
                    max_gas_amount: U64(ut.max_gas_amount as u64),
                    gas_unit_price: U64(ut.gas_unit_price as u64),
                    gas_currency_code: "".to_string(),
                    expiration_timestamp_secs: U64(
                        ut.expiration_timestamp_secs.timestamp_millis() as u64
                    ),
                    payload: serde_json::from_value(tx.payload).unwrap(),
                    signature: serde_json::from_value(ut.signature).unwrap(),
                },
                events,
                timestamp: U64(ut.timestamp.timestamp_millis() as u64),
            }))
        }
        _ => unreachable!("Unknown transaction type: {}", tx.type_),
    };

    Ok(transaction)
}
