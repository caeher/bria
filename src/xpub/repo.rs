use sqlx::{Pool, Postgres, Transaction};
use tracing::instrument;
use uuid::Uuid;

use super::{entity::*, reference::*};
use crate::{entity::*, error::*, primitives::*};

#[derive(Clone)]
pub struct XPubs {
    pool: Pool<Postgres>,
}

impl XPubs {
    pub fn new(pool: &Pool<Postgres>) -> Self {
        Self { pool: pool.clone() }
    }

    #[instrument(name = "xpubs.persist", skip(self))]
    pub async fn persist(&self, xpub: NewAccountXPub) -> Result<XPubId, BriaError> {
        let mut tx = self.pool.begin().await?;
        let ret = self.persist_in_tx(&mut tx, xpub).await?;
        tx.commit().await?;
        Ok(ret)
    }

    #[instrument(name = "xpubs.persist_in_tx", skip(self))]
    pub async fn persist_in_tx(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        xpub: NewAccountXPub,
    ) -> Result<XPubId, BriaError> {
        let xpub_id = xpub.id();
        sqlx::query!(
            r#"INSERT INTO bria_xpubs
            (id, account_id, name, fingerprint)
            VALUES ($1, $2, $3, $4)"#,
            xpub.db_uuid,
            Uuid::from(xpub.account_id),
            xpub.key_name,
            xpub_id.as_bytes()
        )
        .execute(&mut *tx)
        .await?;
        let id = xpub.db_uuid;
        EntityEvents::<XPubEvent>::persist(
            "bria_xpub_events",
            &mut *tx,
            xpub.initial_events().new_serialized_events(id),
        )
        .await?;
        Ok(xpub_id)
    }

    pub async fn persist_updated(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        xpub: AccountXPub,
    ) -> Result<(), BriaError> {
        Ok(EntityEvents::<XPubEvent>::persist(
            "bria_xpub_events",
            tx,
            xpub.events.new_serialized_events(xpub.db_uuid),
        )
        .await?)
    }

    pub async fn find_from_ref(
        &self,
        account_id: AccountId,
        xpub_ref: impl Into<XPubRef>,
    ) -> Result<AccountXPub, BriaError> {
        let xpub_ref = xpub_ref.into();
        let mut tx = self.pool.begin().await?;
        let db_uuid = match xpub_ref {
            XPubRef::Id(fp) => {
                let record = sqlx::query!(
                    r#"SELECT id FROM bria_xpubs WHERE account_id = $1 AND fingerprint = $2"#,
                    Uuid::from(account_id),
                    fp.as_bytes()
                )
                .fetch_one(&mut tx)
                .await?;
                record.id
            }
            XPubRef::Name(name) => {
                let record = sqlx::query!(
                    r#"SELECT id FROM bria_xpubs WHERE account_id = $1 AND name = $2"#,
                    Uuid::from(account_id),
                    name
                )
                .fetch_one(&mut tx)
                .await?;
                record.id
            }
        };

        let rows = sqlx::query!(
            r#"SELECT sequence, event_type, event FROM bria_xpub_events
               WHERE id = $1
               ORDER BY sequence"#,
            db_uuid
        )
        .fetch_all(&mut tx)
        .await?;
        let mut events = EntityEvents::new();
        for row in rows {
            events.load_event(row.sequence as usize, row.event)?;
        }
        Ok(AccountXPub::try_from(events)?)
    }
}
