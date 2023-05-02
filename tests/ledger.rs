mod helpers;

use bdk::BlockTime;
use bria::{
    ledger::*,
    primitives::{bitcoin::*, *},
    wallet::balance::WalletBalanceSummary,
};
use rand::distributions::{Alphanumeric, DistString};

use std::collections::HashMap;

#[tokio::test]
async fn utxo_confirmation() -> anyhow::Result<()> {
    let pool = helpers::init_pool().await?;

    let ledger = Ledger::init(&pool).await?;

    let account_id = AccountId::new();
    let name = Alphanumeric.sample_string(&mut rand::thread_rng(), 32);
    let mut tx = pool.begin().await?;
    let journal_id = ledger
        .create_journal_for_account(&mut tx, account_id, name.clone())
        .await?;
    let wallet_id = WalletId::new();
    let wallet_ledger_accounts = ledger
        .create_ledger_accounts_for_wallet(&mut tx, wallet_id, &name)
        .await?;

    let one_btc = Satoshis::from(100_000_000);
    let one_sat = Satoshis::from(1);
    let zero = Satoshis::from(0);
    let address: bitcoin::Address = "1A1zP1eP5QGefi2DMPTfTL5SLmv7DivfNa".parse().unwrap();
    let outpoint = OutPoint {
        txid: "4010e27ff7dc6d9c66a5657e6b3d94b4c4e394d968398d16fefe4637463d194d"
            .parse()
            .unwrap(),
        vout: 0,
    };

    let keychain_id = KeychainId::new();
    let pending_id = LedgerTransactionId::new();

    ledger
        .utxo_detected(
            tx,
            pending_id,
            UtxoDetectedParams {
                journal_id,
                onchain_incoming_account_id: wallet_ledger_accounts.onchain_incoming_id,
                onchain_fee_account_id: wallet_ledger_accounts.fee_id,
                logical_incoming_account_id: wallet_ledger_accounts.logical_incoming_id,
                meta: UtxoDetectedMeta {
                    account_id,
                    wallet_id,
                    keychain_id,
                    outpoint,
                    satoshis: one_btc,
                    address: address.clone(),
                    encumbered_spending_fee_sats: one_sat,
                    confirmation_time: None,
                },
            },
        )
        .await?;

    let summary = WalletBalanceSummary::from(
        ledger
            .get_wallet_ledger_account_balances(journal_id, wallet_ledger_accounts)
            .await?,
    );

    assert_eq!(summary.pending_incoming_utxos, one_btc);
    assert_eq!(summary.logical_pending_income, one_btc);
    assert_eq!(summary.encumbered_fees, one_sat);

    let confirmed_id = LedgerTransactionId::new();

    let tx = pool.begin().await?;
    ledger
        .utxo_settled(
            tx,
            confirmed_id,
            UtxoSettledParams {
                journal_id,
                ledger_account_ids: wallet_ledger_accounts,
                pending_id,
                meta: UtxoSettledMeta {
                    account_id,
                    wallet_id,
                    keychain_id,
                    outpoint,
                    satoshis: one_btc,
                    address,
                    confirmation_time: BlockTime {
                        height: 1,
                        timestamp: 123409,
                    },
                    already_spent_tx_id: None,
                },
            },
        )
        .await?;

    let summary = WalletBalanceSummary::from(
        ledger
            .get_wallet_ledger_account_balances(journal_id, wallet_ledger_accounts)
            .await?,
    );

    assert_eq!(summary.pending_incoming_utxos, zero);
    assert_eq!(summary.logical_pending_income, zero);
    assert_eq!(summary.settled_utxos, one_btc);
    assert_eq!(summary.logical_settled, one_btc);
    assert_eq!(summary.encumbered_fees, one_sat);

    let reserved_fees = ledger
        .sum_reserved_fees_in_txs(vec![pending_id, confirmed_id].into_iter())
        .await?;
    assert_eq!(reserved_fees, one_sat);

    Ok(())
}

#[tokio::test]
async fn spent_utxo_confirmation() -> anyhow::Result<()> {
    let pool = helpers::init_pool().await?;

    let ledger = Ledger::init(&pool).await?;

    let account_id = AccountId::new();
    let name = Alphanumeric.sample_string(&mut rand::thread_rng(), 32);
    let mut tx = pool.begin().await?;
    let journal_id = ledger
        .create_journal_for_account(&mut tx, account_id, name.clone())
        .await?;
    let wallet_id = WalletId::new();
    let wallet_ledger_accounts = ledger
        .create_ledger_accounts_for_wallet(&mut tx, wallet_id, &name)
        .await?;

    let one_btc = Satoshis::from(100_000_000);
    let one_sat = Satoshis::from(1);
    let zero = Satoshis::from(0);
    let address: bitcoin::Address = "1A1zP1eP5QGefi2DMPTfTL5SLmv7DivfNa".parse().unwrap();
    let outpoint = OutPoint {
        txid: "4010e27ff7dc6d9c66a5657e6b3d94b4c4e394d968398d16fefe4637463d194d"
            .parse()
            .unwrap(),
        vout: 0,
    };

    let keychain_id = KeychainId::new();
    let pending_id = LedgerTransactionId::new();

    ledger
        .utxo_detected(
            tx,
            pending_id,
            UtxoDetectedParams {
                journal_id,
                onchain_incoming_account_id: wallet_ledger_accounts.onchain_incoming_id,
                onchain_fee_account_id: wallet_ledger_accounts.fee_id,
                logical_incoming_account_id: wallet_ledger_accounts.logical_incoming_id,
                meta: UtxoDetectedMeta {
                    account_id,
                    wallet_id,
                    keychain_id,
                    outpoint,
                    satoshis: one_btc,
                    address: address.clone(),
                    encumbered_spending_fee_sats: one_sat,
                    confirmation_time: None,
                },
            },
        )
        .await?;

    let summary = WalletBalanceSummary::from(
        ledger
            .get_wallet_ledger_account_balances(journal_id, wallet_ledger_accounts)
            .await?,
    );

    assert_eq!(summary.pending_incoming_utxos, one_btc);
    assert_eq!(summary.logical_pending_income, one_btc);
    assert_eq!(summary.encumbered_fees, one_sat);

    let confirmed_id = LedgerTransactionId::new();

    let tx = pool.begin().await?;
    ledger
        .utxo_settled(
            tx,
            confirmed_id,
            UtxoSettledParams {
                journal_id,
                ledger_account_ids: wallet_ledger_accounts,
                pending_id,
                meta: UtxoSettledMeta {
                    account_id,
                    wallet_id,
                    keychain_id,
                    outpoint,
                    satoshis: one_btc,
                    address,
                    confirmation_time: BlockTime {
                        height: 1,
                        timestamp: 123409,
                    },
                    already_spent_tx_id: Some(LedgerTransactionId::new()),
                },
            },
        )
        .await?;

    let summary = WalletBalanceSummary::from(
        ledger
            .get_wallet_ledger_account_balances(journal_id, wallet_ledger_accounts)
            .await?,
    );

    assert_eq!(summary.pending_incoming_utxos, zero);
    assert_eq!(summary.logical_pending_income, zero);
    assert_eq!(summary.settled_utxos, zero);

    Ok(())
}

#[tokio::test]
async fn queue_payout() -> anyhow::Result<()> {
    let pool = helpers::init_pool().await?;

    let ledger = Ledger::init(&pool).await?;

    let account_id = AccountId::new();
    let name = Alphanumeric.sample_string(&mut rand::thread_rng(), 32);
    let mut tx = pool.begin().await?;
    let journal_id = ledger
        .create_journal_for_account(&mut tx, account_id, name.clone())
        .await?;
    let wallet_id = WalletId::new();
    let wallet_ledger_accounts = ledger
        .create_ledger_accounts_for_wallet(&mut tx, wallet_id, &name)
        .await?;

    tx.commit().await?;

    let payout_id = PayoutId::new();
    let payout_satoshis = Satoshis::from(50_000_000);

    let tx = pool.begin().await?;
    ledger
        .payout_queued(
            tx,
            LedgerTransactionId::new(),
            PayoutQueuedParams {
                journal_id,
                logical_outgoing_account_id: wallet_ledger_accounts.logical_outgoing_id,
                external_id: payout_id.to_string(),
                payout_satoshis,
                meta: PayoutQueuedMeta {
                    account_id,
                    payout_id,
                    wallet_id,
                    batch_group_id: BatchGroupId::new(),
                    destination: PayoutDestination::OnchainAddress {
                        value: "1A1zP1eP5QGefi2DMPTfTL5SLmv7DivfNa".parse().unwrap(),
                    },
                },
            },
        )
        .await?;

    let summary = WalletBalanceSummary::from(
        ledger
            .get_wallet_ledger_account_balances(journal_id, wallet_ledger_accounts)
            .await?,
    );

    assert_eq!(summary.logical_encumbered_outgoing, payout_satoshis);

    Ok(())
}

#[tokio::test]
async fn create_batch() -> anyhow::Result<()> {
    let pool = helpers::init_pool().await?;

    let ledger = Ledger::init(&pool).await?;

    let account_id = AccountId::new();
    let name = Alphanumeric.sample_string(&mut rand::thread_rng(), 32);
    let mut tx = pool.begin().await?;
    let journal_id = ledger
        .create_journal_for_account(&mut tx, account_id, name.clone())
        .await?;
    let wallet_id = WalletId::new();
    let wallet_ledger_accounts = ledger
        .create_ledger_accounts_for_wallet(&mut tx, wallet_id, &name)
        .await?;

    tx.commit().await?;

    let batch_id = BatchId::new();
    let fee_sats = Satoshis::from(2_346);
    let total_spent_sats = Satoshis::from(100_000_000);
    let total_utxo_in_sats = Satoshis::from(200_000_000);
    let change_sats = total_utxo_in_sats - total_spent_sats - fee_sats;
    let encumbered_fees = Satoshis::from(12_346);

    let tx = pool.begin().await?;
    ledger
        .batch_created(
            tx,
            LedgerTransactionId::new(),
            BatchCreatedParams {
                journal_id,
                ledger_account_ids: wallet_ledger_accounts,
                encumbered_fees,
                meta: BatchCreatedMeta {
                    batch_info: BatchWalletInfo {
                        account_id,
                        wallet_id,
                        batch_id,
                        batch_group_id: BatchGroupId::new(),
                        included_payouts: Vec::new(),
                    },
                    tx_summary: WalletTransactionSummary {
                        account_id,
                        wallet_id,
                        bitcoin_tx_id:
                            "4010e27ff7dc6d9c66a5657e6b3d94b4c4e394d968398d16fefe4637463d194d"
                                .parse()
                                .unwrap(),
                        total_utxo_settled_in_sats: total_utxo_in_sats,
                        total_utxo_in_sats,
                        change_sats,
                        fee_sats,
                        change_address: None,
                        change_outpoint: None,
                        current_keychain_id: KeychainId::new(),
                    },
                },
            },
        )
        .await?;

    let balances = ledger
        .get_wallet_ledger_account_balances(journal_id, wallet_ledger_accounts)
        .await?;
    let summary = WalletBalanceSummary::from(balances);

    assert_eq!(summary.logical_pending_outgoing, total_spent_sats);
    assert_eq!(
        summary.logical_settled.flip_sign(),
        total_spent_sats + fee_sats
    );
    assert_eq!(
        summary.logical_encumbered_outgoing.flip_sign(),
        total_spent_sats
    );
    assert_eq!(summary.encumbered_fees.flip_sign(), encumbered_fees);
    assert_eq!(summary.pending_fees, fee_sats);
    assert_eq!(
        summary.encumbered_incoming_utxos,
        total_utxo_in_sats - fee_sats - total_spent_sats
    );
    assert_eq!(summary.settled_utxos.flip_sign(), total_utxo_in_sats);
    assert_eq!(
        summary.pending_outgoing_utxos,
        total_utxo_in_sats - fee_sats
    );

    Ok(())
}

#[tokio::test]
async fn spend_detected() -> anyhow::Result<()> {
    let pool = helpers::init_pool().await?;

    let ledger = Ledger::init(&pool).await?;

    let account_id = AccountId::new();
    let name = Alphanumeric.sample_string(&mut rand::thread_rng(), 32);
    let mut tx = pool.begin().await?;
    let journal_id = ledger
        .create_journal_for_account(&mut tx, account_id, name.clone())
        .await?;
    let wallet_id = WalletId::new();
    let keychain_id = KeychainId::new();
    let wallet_ledger_accounts = ledger
        .create_ledger_accounts_for_wallet(&mut tx, wallet_id, &name)
        .await?;

    tx.commit().await?;

    let fee_sats = Satoshis::from(2_346);
    let change_sats = Satoshis::from(40_000_000);
    let total_utxo_in_sats = Satoshis::from(200_000_000);
    let total_utxo_settled_in_sats = Satoshis::from(200_000_000);
    let reserved_fees = Satoshis::from(12_346);
    let encumbered_spending_fee_sats = Satoshis::ONE;

    let pending_id = LedgerTransactionId::new();
    let tx = pool.begin().await?;
    ledger
        .spend_detected(
            tx,
            pending_id,
            SpendDetectedParams {
                journal_id,
                ledger_account_ids: wallet_ledger_accounts,
                reserved_fees,
                meta: SpendDetectedMeta {
                    withdraw_from_logical_when_settled: HashMap::new(),
                    tx_summary: WalletTransactionSummary {
                        account_id,
                        wallet_id,
                        current_keychain_id: keychain_id,
                        bitcoin_tx_id:
                            "4010e27ff7dc6d9c66a5657e6b3d94b4c4e394d968398d16fefe4637463d194d"
                                .parse()
                                .unwrap(),
                        total_utxo_in_sats,
                        total_utxo_settled_in_sats,
                        change_sats,
                        fee_sats,
                        change_address: None,
                        change_outpoint: None,
                    },
                    encumbered_spending_fee_sats: Some(encumbered_spending_fee_sats),
                    confirmation_time: None,
                },
            },
        )
        .await?;

    let balances = ledger
        .get_wallet_ledger_account_balances(journal_id, wallet_ledger_accounts)
        .await?;
    let summary = WalletBalanceSummary::from(balances);

    assert_eq!(
        summary.logical_pending_outgoing,
        total_utxo_in_sats - fee_sats - change_sats
    );
    assert_eq!(
        summary.logical_settled.flip_sign(),
        total_utxo_in_sats - change_sats
    );
    assert_eq!(
        summary.encumbered_fees.flip_sign(),
        reserved_fees - encumbered_spending_fee_sats
    );
    assert_eq!(summary.pending_fees, fee_sats);
    assert_eq!(
        summary.settled_utxos.flip_sign(),
        total_utxo_settled_in_sats
    );
    assert_eq!(
        summary.pending_outgoing_utxos,
        total_utxo_in_sats - fee_sats
    );
    assert_eq!(summary.pending_incoming_utxos, change_sats);

    let tx = pool.begin().await?;
    ledger
        .spend_settled(
            tx,
            LedgerTransactionId::new(),
            journal_id,
            wallet_ledger_accounts,
            pending_id,
            BlockTime {
                height: 2,
                timestamp: 123409,
            },
            false,
        )
        .await?;

    let balances = ledger
        .get_wallet_ledger_account_balances(journal_id, wallet_ledger_accounts)
        .await?;
    let summary = WalletBalanceSummary::from(balances);
    assert_eq!(summary.logical_pending_outgoing, Satoshis::ZERO);
    assert_eq!(
        summary.logical_settled.flip_sign(),
        total_utxo_in_sats - change_sats
    );
    assert_eq!(summary.pending_fees, Satoshis::ZERO);
    assert_eq!(
        summary.settled_utxos.flip_sign(),
        total_utxo_in_sats - change_sats
    );
    assert_eq!(summary.pending_outgoing_utxos, Satoshis::ZERO);
    assert_eq!(summary.pending_incoming_utxos, Satoshis::ZERO);

    Ok(())
}

#[tokio::test]
async fn spend_detected_unconfirmed() -> anyhow::Result<()> {
    let pool = helpers::init_pool().await?;

    let ledger = Ledger::init(&pool).await?;

    let account_id = AccountId::new();
    let name = Alphanumeric.sample_string(&mut rand::thread_rng(), 32);
    let mut tx = pool.begin().await?;
    let journal_id = ledger
        .create_journal_for_account(&mut tx, account_id, name.clone())
        .await?;
    let wallet_id = WalletId::new();
    let keychain_id = KeychainId::new();
    let wallet_ledger_accounts = ledger
        .create_ledger_accounts_for_wallet(&mut tx, wallet_id, &name)
        .await?;

    tx.commit().await?;

    let fee_sats = Satoshis::from(2_346);
    let change_sats = Satoshis::from(40_000_000);
    let total_utxo_in_sats = Satoshis::from(200_000_000);
    let total_utxo_settled_in_sats = Satoshis::from(100_000_000);
    let outpoint = OutPoint {
        txid: "4010e27ff7dc6d9c66a5657e6b3d94b4c4e394d968398d16fefe4637463d194d"
            .parse()
            .unwrap(),
        vout: 0,
    };
    let deferred_sats = Satoshis::from(50_000);
    let withdraw_from_logical_when_settled = std::iter::once((outpoint, deferred_sats)).collect();
    let reserved_fees = Satoshis::from(12_346);
    let encumbered_spending_fee_sats = Satoshis::ONE;

    let pending_id = LedgerTransactionId::new();
    let tx = pool.begin().await?;
    ledger
        .spend_detected(
            tx,
            pending_id,
            SpendDetectedParams {
                journal_id,
                ledger_account_ids: wallet_ledger_accounts,
                reserved_fees,
                meta: SpendDetectedMeta {
                    withdraw_from_logical_when_settled,
                    tx_summary: WalletTransactionSummary {
                        account_id,
                        wallet_id,
                        current_keychain_id: keychain_id,
                        bitcoin_tx_id:
                            "4010e27ff7dc6d9c66a5657e6b3d94b4c4e394d968398d16fefe4637463d194d"
                                .parse()
                                .unwrap(),
                        total_utxo_in_sats,
                        total_utxo_settled_in_sats,
                        change_sats,
                        fee_sats,
                        change_address: None,
                        change_outpoint: None,
                    },
                    encumbered_spending_fee_sats: Some(encumbered_spending_fee_sats),
                    confirmation_time: None,
                },
            },
        )
        .await?;

    let balances = ledger
        .get_wallet_ledger_account_balances(journal_id, wallet_ledger_accounts)
        .await?;
    let summary = WalletBalanceSummary::from(balances);

    assert_eq!(
        summary.logical_pending_outgoing,
        total_utxo_in_sats - fee_sats - change_sats
    );
    assert_eq!(
        summary.logical_settled.flip_sign(),
        total_utxo_in_sats - change_sats - deferred_sats
    );
    assert_eq!(
        summary.settled_utxos.flip_sign(),
        total_utxo_settled_in_sats
    );
    assert_eq!(
        summary.pending_outgoing_utxos,
        total_utxo_in_sats - fee_sats
    );
    assert_eq!(summary.pending_incoming_utxos, change_sats);

    Ok(())
}
