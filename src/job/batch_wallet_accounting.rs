use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::instrument;

use crate::{
    app::BlockchainConfig, batch::*, error::*, ledger::*, primitives::*, utxo::*, wallet::*,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchWalletAccountingData {
    pub(super) account_id: AccountId,
    pub(super) wallet_id: WalletId,
    pub(super) batch_id: BatchId,
    #[serde(flatten)]
    pub(super) tracing_data: HashMap<String, String>,
}

#[instrument(
    name = "job.batch_wallet_accounting",
    skip(wallets, batches, ledger, bria_utxos),
    err
)]
pub async fn execute(
    data: BatchWalletAccountingData,
    blockchain_cfg: BlockchainConfig,
    ledger: Ledger,
    wallets: Wallets,
    bria_utxos: Utxos,
    batches: Batches,
) -> Result<BatchWalletAccountingData, BriaError> {
    let Batch {
        id,
        bitcoin_tx_id,
        batch_group_id,
        wallet_summaries,
        included_utxos,
        ..
    } = batches.find_by_id(data.account_id, data.batch_id).await?;

    let wallet_summary = wallet_summaries
        .get(&data.wallet_id)
        .expect("wallet summary not found");
    let wallet = wallets.find_by_id(data.wallet_id).await?;

    let utxos = included_utxos
        .get(&data.wallet_id)
        .expect("utxos not found");
    let utxos = bria_utxos.list_utxos_by_outpoint(utxos).await?;
    let encumbered_fees = ledger
        .sum_reserved_fees_in_txs(utxos.into_iter().map(|u| u.income_detected_ledger_tx_id))
        .await?;

    if let Some((tx, tx_id)) = batches
        .set_batch_created_ledger_tx_id(data.batch_id, data.wallet_id)
        .await?
    {
        ledger
            .batch_created(
                tx,
                tx_id,
                BatchCreatedParams {
                    journal_id: wallet.journal_id,
                    ledger_account_ids: wallet.ledger_account_ids,
                    encumbered_fees,
                    meta: BatchCreatedMeta {
                        batch_id: id,
                        batch_group_id,
                        tx_summary: WalletTransactionSummary {
                            account_id: data.account_id,
                            wallet_id: wallet_summary.wallet_id,
                            current_keychain_id: wallet_summary.change_keychain_id,
                            fee_sats: wallet_summary.fee_sats,
                            bitcoin_tx_id,
                            total_utxo_in_sats: wallet_summary.total_in_sats,
                            total_utxo_settled_in_sats: wallet_summary.total_in_sats,
                            change_sats: wallet_summary.change_sats,
                            change_outpoint: wallet_summary.change_outpoint,
                            change_address: wallet_summary
                                .change_outpoint
                                .as_ref()
                                .map(|_| wallet_summary.change_address.clone()),
                        },
                    },
                },
            )
            .await?;
    }
    Ok(data)
}
