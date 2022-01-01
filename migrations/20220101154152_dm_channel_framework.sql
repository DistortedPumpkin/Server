-- Add migration script here
CREATE TABLE dmchannels (
    id NUMERIC(39) PRIMARY KEY,
    is_group BOOL NOT NULL,
    name VARCHAR(100)
);

CREATE TABLE dmmembers (
    dm_id NUMERIC(39) REFERENCES dmchannels,
    user_id NUMERIC(39) REFERENCES users
);
