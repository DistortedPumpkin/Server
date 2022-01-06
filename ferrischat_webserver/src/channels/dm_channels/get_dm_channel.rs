use ferrischat_common::types::{User, DMChannel, UserFlags};
use axum::extract::{Path, Json};

/// GET `/v0/users/me/channels/{channel_id}`
pub async fn get_dm_channel(
    auth: crate::Authorization,
    Path(channel_id): Path<u128>,
) -> Result<Json<DMChannel>, WebServerError> {
    let db = get_db_or_fail!();

    let bigint_authed_user = u128_to_bigdecimal!(auth.0);
    let bigint_channel_id = u128_to_bigdecimal!(channel_id);

    let res_dm_channel = sqlx::query!(r"#
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
        WHERE c.id = $1 AND EXISTS (
            SELECT * FROM dmmembers 
            WHERE user_id = $2 AND dm_id = c.id
        )
    #",
        bigint_channel_id,
        bigint_authed_user
    )
    .fetch_optional(db)
    .await?;

    match res_dm_channel {
        None => {
            return Err(ErrorJson::new_404(format!("Unknown private channel with ID {}", channel_id)).into());
        },
        Some(channel) => {
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

            let dm_channel_obj = DMChannel {
                id: channel_id,
                name: channel.name.clone(),
                users,
                group: channel.is_group,
            }
            
            return Ok(crate::Json {
                obj: dm_channel_obj,
                code: 200,
            })
        }
    }
}