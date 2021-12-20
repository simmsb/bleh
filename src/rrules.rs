use std::convert::TryInto;

use matrix_sdk::{
    ruma::{events::room::message::RoomMessageEventContent, RoomId, UserId},
    Client,
};
use sqlx::SqlitePool;

#[derive(sqlx::FromRow)]
pub struct RRule {
    pub id: i64,
    pub rule: String,
    pub message: String,
    pub channel: String,
    pub userid: String,
}

impl RRule {
    pub async fn perform(&self, client: Client) {
        let rrule: rrule::RRule = self
            .rule
            .parse()
            .expect("These should be validated earlier");
        let room_id = RoomId::parse(self.channel.as_str()).unwrap();
        let user_id = UserId::parse(self.userid.as_str()).unwrap();

        let now = chrono::offset::Utc::now();
        for dt in &rrule {
            if dt < now {
                continue;
            }

            println!("rrule until: {}", dt);

            let utcnow = chrono::offset::Utc::now().timestamp();
            let delta =
                std::time::Duration::from_secs((dt.timestamp() - utcnow).try_into().unwrap());
            tokio::time::sleep(delta).await;

            let joined = match client.get_joined_room(&room_id) {
                Some(joined) => joined,
                None => return,
            };

            let m = RoomMessageEventContent::text_plain(&format!(
                "{}: {}",
                user_id.as_str(),
                self.message
            ));

            let _ = joined.send(m, None).await;
        }
    }
}

pub async fn setup(client: Client, pool: &SqlitePool) {
    let rows = sqlx::query_as!(RRule, "SELECT * FROM rrules")
        .fetch_all(pool)
        .await
        .unwrap();

    for r in rows {
        let client = client.clone();
        tokio::spawn(async move {
            r.perform(client).await;
        });
    }
}
