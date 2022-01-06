use ferrischat_common::types::{User, DMChannel, UserFlags};
use axum::extract::{Json, Query};
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
            ARRAY(
                SELECT ARRAY[
                    CAST(id AS VARCHAR(39)), 
                    name, 
                    avatar, 
                    CAST(flags AS VARCHAR(39)), 
                    CAST(discriminator AS VARCHAR(39)),
                    CAST(pronouns AS VARCHAR(39))
                ] AS u
                FROM users 
                WHERE id IN (
                    SELECT user_id FROM dmmembers WHERE dm_id = c.id
                )
            ) AS _users
        FROM 
            dmchannels c
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

    let mut dm_channels = Vec::with_capacity(res_dm_channels.len());
    for dm in res_dm_channels {
        let mut users = Vec::with_capacity(res_dm_channel.len());

        for user in channel._users {
            users.push(User {
                id: str::parse::<u128>(user[0]),
                name: users[1].clone(),
                avatar: users[2].clone(),
                guilds: None,
                flags: UserFlags::from_bits_truncate(str::parse::<i64>(users[3])),
                discriminator: str::parse::<i16>(users[4]),
                pronouns: str::parse::<i16>(users[5])
                    .and_then(ferrischat_common::types::Pronouns::from_i16),
            });
        }

        dm_channels.push(DMChannel {
            id: dm.id,
            name: dm.name.clone(),
            users,
            group: dm.is_group
        })
    }

    Ok(crate::Json {
        obj: dm_channels,
        code: 200,
    })
}