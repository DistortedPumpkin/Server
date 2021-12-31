use ferrischat_common::types::{User, DMChannel, UserFlags};
use axum::extract::{Path, Json, Query};
use std::collections::HashMap;
use ferrischat_common::request_json::GetSelfDmChannelParams;


/// GET `/v0/users/me/channels`
pub async fn get_self_dm_channels(
    auth: crate::Authorization,
    Query(GetSelfDmChannelParams { limit }): Query<GetSelfDmChannelParams>,
) -> Result<Json<Vec<DMChannel>>, WebServerError> {
    let db = get_db_or_fail!();

    let bigint_authed_user = u128_to_bigdecimal!(auth.0);

    let res_dm_channels = sqlx::query!(r#"
        SELECT 
            c.*, 
            u.id AS user_id, 
            u.name AS user_name, 
            u.avatar AS user_avatar, 
            u.discriminator AS user_discriminator, 
            u.flags AS user_flags, 
            u.pronouns AS user_pronouns
        FROM 
            dmchannels c
        LEFT JOIN users u 
        ON u.id IN (
            SELECT user_id FROM dmmembers 
            WHERE dm_id = c.id
        )
        WHERE EXISTS (
            SELECT * FROM dmmembers 
            WHERE user_id = $1 AND dm_id = c.id
        )
        LIMIT $2;
    "#, 
        bigint_authed_user,
        limit.unwrap_or(128)
    )
    .fetch_all(db)
    .await?;

    let mut dm_channel_users = HashMap::new();
    for dm in res_dm_channels {
        match dm_channel_users.get(dm.id) {
            None => {
                dm_channel_users.insert(dm.id, Vec::new())
            },
            Some(users) => {
                users.push(User {
                    id: bigdecimal_to_u128!(dm.user_id),
                    name: dm.user_name.clone(),
                    avatar: dm.user_avatar,
                    guilds: None,
                    flags: UserFlags::from_bits_truncate(dm.user_flags),
                    discriminator: dm.user_discriminator,
                    pronouns: dm
                        .user_pronouns
                        .and_then(ferrischat_common::types::Pronouns::from_i16),
                })
            }
        }
    }

    let mut dm_channels = Vec::with_capacity(res_dm_channels.len());
    for dm in res_dm_channels {
        dm_channels.push(DMChannel {
            id: bigdecimal_to_u128!(dm.id),
            name: dm.name.clone(),
            users: dm_channel_users.get(dm.id),
            group: dm.is_group,
        });
    }

    Ok(crate::Json {
        obj: dm_channels,
        code: 200,
    })
}