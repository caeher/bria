use bdk::bitcoin::{Transaction, Txid};
use serde::{Deserialize, Serialize};

/// A wallet transaction
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct TransactionDetails {
    /// Transaction id
    pub txid: Txid,
    /// The actual transaction data, if available
    pub transaction: Option<Transaction>,
    /// Total amount received in satoshis (sum of owned outputs)
    pub received: u64,
    /// Total amount sent in satoshis (sum of owned inputs)
    pub sent: u64,
    /// Transaction fee in satoshis, if confirmed
    /// Note: Always available with Electrum Server backend, may be None with Bitcoin RPC node without txindex
    pub fee: Option<u64>,
    /// Block confirmation details if confirmed, None if unconfirmed
    pub confirmation_time: Option<BlockTime>,
}

impl PartialOrd for TransactionDetails {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for TransactionDetails {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.confirmation_time
            .cmp(&other.confirmation_time)
            .then_with(|| self.txid.cmp(&other.txid))
    }
}

/// Block height and timestamp information
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Default)]
pub struct BlockTime {
    /// Block height
    pub height: u32,
    /// Block timestamp (Unix timestamp)
    pub timestamp: u64,
}

impl BlockTime {
    /// Creates a new BlockTime if both height and timestamp are provided
    pub fn new(height: Option<u32>, timestamp: Option<u64>) -> Option<Self> {
        match (height, timestamp) {
            (Some(height), Some(timestamp)) => Some(BlockTime { height, timestamp }),
            _ => None,
        }
    }
}

impl PartialOrd for BlockTime {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for BlockTime {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.height
            .cmp(&other.height)
            .then_with(|| self.timestamp.cmp(&other.timestamp))
    }
}