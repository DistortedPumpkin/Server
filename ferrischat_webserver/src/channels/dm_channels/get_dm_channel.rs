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
        WHERE c.id = $1 AND EXISTS (
            SELECT * FROM dmmembers 
            WHERE user_id = $2 AND dm_id = c.id
        )
    #",
        bigint_channel_id,
        bigint_authed_user
    )
    .fetch_all(db)
    .await?;

    if res_dm_channel.len() == 0 {
        return Err(ErrorJson::new_404(format!("Unknown private channel with ID {}", channel_id)).into());
    }

    let mut users = Vec::with_capacity(res_dm_channel.len());

    for dm in res_dm_channel {
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
        });
    }

    let dm_channel_obj = DMChannel {
        id: channel_id,
        name: res_dm_channel[0].name.clone(),
        users,
        group: res_dm_channel[0].is_group,
    }

    Ok(crate::Json {
        obj: dm_channel_obj,
        code: 200,
    })
    
}