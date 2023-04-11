use sqlx::{Pool, Postgres, QueryBuilder, Row, Transaction};
use uuid::Uuid;

use std::collections::HashMap;

use super::entity::*;
use crate::{
    error::*,
    primitives::{bitcoin::*, *},
};

pub struct ReservableUtxo {
    pub keychain_id: KeychainId,
    pub income_address: bool,
    pub outpoint: OutPoint,
    pub spending_batch_id: Option<BatchId>,
    pub confirmed_income_ledger_tx_id: Option<LedgerTransactionId>,
}

#[derive(Clone)]
pub(super) struct UtxoRepo {
    pool: Pool<Postgres>,
}

impl UtxoRepo {
    pub fn new(pool: Pool<Postgres>) -> Self {
        Self { pool }
    }

    pub async fn persist_utxo(
        &self,
        utxo: NewUtxo,
    ) -> Result<Option<(LedgerTransactionId, Transaction<'_, Postgres>)>, BriaError> {
        let mut tx = self.pool.begin().await?;
        let result = sqlx::query!(
            r#"INSERT INTO bria_utxos
               (wallet_id, keychain_id, tx_id, vout, sats_per_vbyte_when_created, self_pay, kind, address_idx, value, address, script_hex, pending_income_ledger_tx_id)
               VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
               ON CONFLICT (keychain_id, tx_id, vout) DO NOTHING"#,
          Uuid::from(utxo.wallet_id),
          Uuid::from(utxo.keychain_id),
          utxo.outpoint.txid.to_string(),
          utxo.outpoint.vout as i32,
          utxo.sats_per_vbyte_when_created,
          utxo.self_pay,
          pg::PgKeychainKind::from(utxo.kind) as pg::PgKeychainKind,
          utxo.address_idx as i32,
          utxo.value.into_inner(),
          utxo.address.to_string(),
          utxo.script_hex,
          Uuid::from(utxo.income_pending_ledger_tx_id)
        )
        .execute(&mut *tx)
        .await?;

        Ok(if result.rows_affected() > 0 {
            Some((utxo.income_pending_ledger_tx_id, tx))
        } else {
            None
        })
    }

    pub async fn mark_utxo_confirmed(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        keychain_id: KeychainId,
        outpoint: OutPoint,
        bdk_spent: bool,
        block_height: u32,
    ) -> Result<ConfirmedUtxo, BriaError> {
        let new_confirmed_ledger_tx_id = LedgerTransactionId::new();

        let row = sqlx::query!(
            r#"UPDATE bria_utxos
            SET bdk_spent = $1,
                block_height = $2,
                confirmed_income_ledger_tx_id = $3,
                modified_at = NOW()
            WHERE keychain_id = $4
              AND tx_id = $5
              AND vout = $6
            RETURNING address_idx, value, address, pending_income_ledger_tx_id, pending_spend_ledger_tx_id"#,
            bdk_spent,
            block_height as i32,
            Uuid::from(new_confirmed_ledger_tx_id),
            Uuid::from(keychain_id),
            outpoint.txid.to_string(),
            outpoint.vout as i32,
        )
        .fetch_one(&mut *tx)
        .await?;

        Ok(ConfirmedUtxo {
            keychain_id,
            value: Satoshis::from(row.value),
            address: row.address.parse().expect("couldn't parse address"),
            pending_income_ledger_tx_id: LedgerTransactionId::from(row.pending_income_ledger_tx_id),
            confirmed_income_ledger_tx_id: new_confirmed_ledger_tx_id,
            pending_spend_ledger_tx_id: row
                .pending_spend_ledger_tx_id
                .map(LedgerTransactionId::from),
        })
    }

    pub async fn mark_spent(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        keychain_id: KeychainId,
        utxos: impl Iterator<Item = &OutPoint>,
        tx_id: LedgerTransactionId,
    ) -> Result<Vec<SpentUtxo>, BriaError> {
        let mut query_builder: QueryBuilder<Postgres> = QueryBuilder::new(
            r#"WITH updated AS ( UPDATE bria_utxos
            SET bdk_spent = true, modified_at = NOW(), pending_spend_ledger_tx_id = "#,
        );
        query_builder.push_bind(Uuid::from(tx_id));
        query_builder
            .push("WHERE pending_spend_ledger_tx_id IS NULL AND (keychain_id, tx_id, vout) IN");
        let mut n_inputs = 0;
        query_builder.push_tuples(utxos, |mut builder, out| {
            n_inputs += 1;
            builder.push_bind(Uuid::from(keychain_id));
            builder.push_bind(out.txid.to_string());
            builder.push_bind(out.vout as i32);
        });
        query_builder.push(
            r#"RETURNING tx_id, vout, value, kind, sats_per_vbyte_when_created, CASE WHEN confirmed_income_ledger_tx_id IS NOT NULL THEN value ELSE 0 END as settled_value )
            SELECT tx_id, vout, value,
                CASE WHEN settled_value != 0 THEN true ELSE false END as confirmed,
                CASE WHEN kind = 'internal' THEN true ELSE false END as change_address
                FROM updated ORDER BY settled_value DESC, sats_per_vbyte_when_created DESC"#
        );

        let query = query_builder.build();
        let res = query.fetch_all(&mut *tx).await?;
        Ok(if n_inputs == res.len() {
            res.into_iter()
                .map(|row| SpentUtxo {
                    outpoint: OutPoint {
                        txid: row.get::<String, _>("tx_id").parse().unwrap(),
                        vout: row.get::<i32, _>("vout") as u32,
                    },
                    value: Satoshis::from(row.get::<rust_decimal::Decimal, _>("value")),
                    confirmed: row.get("confirmed"),
                    change_address: row.get("change_address"),
                })
                .collect()
        } else {
            Vec::new()
        })
    }

    pub async fn confirm_spend(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        keychain_id: KeychainId,
        utxos: impl Iterator<Item = &OutPoint>,
        tx_id: LedgerTransactionId,
    ) -> Result<Option<LedgerTransactionId>, BriaError> {
        let keychain_id = Uuid::from(keychain_id);
        let mut query_builder: QueryBuilder<Postgres> = QueryBuilder::new(
            r#"UPDATE bria_utxos
            SET modified_at = NOW(), confirmed_spend_ledger_tx_id = "#,
        );
        query_builder.push_bind(Uuid::from(tx_id));
        query_builder
            .push("WHERE confirmed_spend_ledger_tx_id IS NULL AND (keychain_id, tx_id, vout) IN");
        let mut rows = 0;
        query_builder.push_tuples(utxos, |mut builder, out| {
            rows += 1;
            builder.push_bind(keychain_id);
            builder.push_bind(out.txid.to_string());
            builder.push_bind(out.vout as i32);
        });
        query_builder.push("RETURNING pending_spend_ledger_tx_id");
        let query = query_builder.build();
        let res = query.fetch_all(&mut *tx).await?;
        Ok(if rows == res.len() {
            Some(LedgerTransactionId::from(
                res[0].get::<Uuid, _>("pending_spend_ledger_tx_id"),
            ))
        } else {
            None
        })
    }

    pub async fn find_keychain_utxos(
        &self,
        keychain_ids: impl Iterator<Item = KeychainId>,
    ) -> Result<HashMap<KeychainId, KeychainUtxos>, BriaError> {
        let keychain_ids: Vec<Uuid> = keychain_ids.map(Uuid::from).collect();
        let rows = sqlx::query!(
            r#"SELECT wallet_id, keychain_id, tx_id, vout, kind as "kind: pg::PgKeychainKind", address_idx, value, address, bdk_spent,
                  CASE
                      WHEN kind = 'external' THEN address
                      ELSE NULL
                  END as optional_address,
                  block_height, pending_income_ledger_tx_id, confirmed_income_ledger_tx_id, spending_batch_id
           FROM bria_utxos
           WHERE keychain_id = ANY($1) AND bdk_spent = false
           ORDER BY created_at DESC"#,
           &keychain_ids
        )
            .fetch_all(&self.pool)
            .await?;

        let mut utxos = HashMap::new();

        for row in rows {
            let utxo = WalletUtxo {
                wallet_id: row.wallet_id.into(),
                keychain_id: row.keychain_id.into(),
                outpoint: OutPoint {
                    txid: row.tx_id.parse().unwrap(),
                    vout: row.vout as u32,
                },
                kind: KeychainKind::from(row.kind),
                address_idx: row.address_idx as u32,
                value: Satoshis::from(row.value),
                address: row
                    .optional_address
                    .map(|addr| addr.parse().expect("couldn't parse address")),
                bdk_spent: row.bdk_spent,
                block_height: row.block_height.map(|v| v as u32),
                pending_income_ledger_tx_id: LedgerTransactionId::from(
                    row.pending_income_ledger_tx_id,
                ),
                confirmed_income_ledger_tx_id: row
                    .confirmed_income_ledger_tx_id
                    .map(LedgerTransactionId::from),
                spending_batch_id: row.spending_batch_id.map(BatchId::from),
            };

            let keychain_id = KeychainId::from(row.keychain_id);
            utxos
                .entry(keychain_id)
                .or_insert_with(|| KeychainUtxos {
                    keychain_id,
                    utxos: Vec::new(),
                })
                .utxos
                .push(utxo);
        }

        Ok(utxos)
    }

    pub async fn find_reservable_utxos(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        ids: impl Iterator<Item = KeychainId>,
    ) -> Result<Vec<ReservableUtxo>, BriaError> {
        let uuids = ids.into_iter().map(Uuid::from).collect::<Vec<_>>();
        let rows = sqlx::query!(
            r#"SELECT keychain_id,
               CASE WHEN kind = 'external' THEN true ELSE false END as income_address,
               tx_id, vout, spending_batch_id, confirmed_income_ledger_tx_id
               FROM bria_utxos
               WHERE keychain_id = ANY($1) AND bdk_spent = false
               FOR UPDATE"#,
            &uuids[..]
        )
        .fetch_all(&mut *tx)
        .await?;

        let reservable_utxos = rows
            .into_iter()
            .map(|row| ReservableUtxo {
                keychain_id: KeychainId::from(row.keychain_id),
                income_address: row.income_address.unwrap_or_default(),
                outpoint: OutPoint {
                    txid: row.tx_id.parse().unwrap(),
                    vout: row.vout as u32,
                },
                spending_batch_id: row.spending_batch_id.map(BatchId::from),
                confirmed_income_ledger_tx_id: row
                    .confirmed_income_ledger_tx_id
                    .map(LedgerTransactionId::from),
            })
            .collect();

        Ok(reservable_utxos)
    }

    pub async fn reserve_utxos_in_batch(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        batch_id: BatchId,
        utxos: impl Iterator<Item = (KeychainId, OutPoint)>,
    ) -> Result<(), BriaError> {
        let mut query_builder: QueryBuilder<Postgres> = QueryBuilder::new(
            r#"UPDATE bria_utxos
            SET spending_batch_id = "#,
        );
        query_builder.push_bind(Uuid::from(batch_id));
        query_builder.push("WHERE (keychain_id, tx_id, vout) IN");
        query_builder.push_tuples(
            utxos.map(|(keychain_id, utxo)| {
                (
                    Uuid::from(keychain_id),
                    utxo.txid.to_string(),
                    utxo.vout as i32,
                )
            }),
            |mut builder, (keychain_id, tx_id, vout)| {
                builder.push_bind(keychain_id);
                builder.push_bind(tx_id);
                builder.push_bind(vout);
            },
        );

        let query = query_builder.build();
        query.execute(&mut *tx).await?;
        Ok(())
    }

    pub async fn list_utxos_by_outpoint(
        &self,
        utxos: &HashMap<KeychainId, Vec<OutPoint>>,
    ) -> Result<Vec<WalletUtxo>, BriaError> {
        let mut query_builder: QueryBuilder<Postgres> = QueryBuilder::new(
            r#"SELECT wallet_id, keychain_id, tx_id, vout, kind, address_idx, value, address, bdk_spent,
                  CASE
                      WHEN kind = 'external' THEN address
                      ELSE NULL
                  END as optional_address,
                  block_height, pending_income_ledger_tx_id, confirmed_income_ledger_tx_id, spending_batch_id
            FROM bria_utxos
            WHERE (keychain_id, tx_id, vout) IN"#,
        );

        query_builder.push_tuples(
            utxos.iter().flat_map(|(keychain_id, utxos)| {
                utxos.iter().map(move |utxo| {
                    (
                        Uuid::from(*keychain_id),
                        utxo.txid.to_string(),
                        utxo.vout as i32,
                    )
                })
            }),
            |mut builder, (keychain_id, tx_id, vout)| {
                builder.push_bind(keychain_id);
                builder.push_bind(tx_id);
                builder.push_bind(vout);
            },
        );
        query_builder
            .push("ORDER BY block_height ASC NULLS LAST, sats_per_vbyte_when_created DESC");
        let query = query_builder.build();
        let rows = query.fetch_all(&self.pool).await?;

        Ok(rows
            .into_iter()
            .map(|row| WalletUtxo {
                wallet_id: WalletId::from(row.get::<Uuid, _>("wallet_id")),
                keychain_id: KeychainId::from(row.get::<Uuid, _>("keychain_id")),
                address: row
                    .get::<Option<String>, _>("optional_address")
                    .map(|addr| addr.parse().expect("couldn't parse address")),
                address_idx: row.get::<i32, _>("address_idx") as u32,
                outpoint: OutPoint {
                    txid: row.get::<String, _>("tx_id").parse().unwrap(),
                    vout: row.get::<i32, _>("vout") as u32,
                },
                kind: KeychainKind::from(row.get::<bitcoin::pg::PgKeychainKind, _>("kind")),
                bdk_spent: row.get("bdk_spent"),
                value: Satoshis::from(row.get::<rust_decimal::Decimal, _>("value")),
                pending_income_ledger_tx_id: LedgerTransactionId::from(
                    row.get::<Uuid, _>("pending_income_ledger_tx_id"),
                ),
                confirmed_income_ledger_tx_id: row
                    .get::<Option<Uuid>, _>("confirmed_income_ledger_tx_id")
                    .map(LedgerTransactionId::from),
                spending_batch_id: row
                    .get::<Option<Uuid>, _>("spending_batch_id")
                    .map(BatchId::from),
                block_height: row.get::<Option<i32>, _>("block_height").map(|h| h as u32),
            })
            .collect())
    }
}