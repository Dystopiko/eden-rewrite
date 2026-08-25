CREATE TYPE challenge_method_enum AS enum (
    'oauth',
    'code'
);

CREATE TYPE challenge_status_enum AS enum (
    'cancelled',
    'in_progress',
    'done'
);

CREATE TYPE mc_edition_enum AS enum (
    'java',
    'bedrock'
);

CREATE TABLE mc_link_challenges (
    id uuid PRIMARY KEY NOT NULL,
    created_at timestamptz NOT NULL,

    method challenge_method_enum NOT NULL,
    expires_at timestamptz NOT NULL,
    hashed_code varchar(255),
    
    player_uuid uuid NOT NULL,
    username varchar(20) NOT NULL,
    edition mc_edition_enum NOT NULL,
    ip_address inet NOT NULL,

    status challenge_status_enum NOT NULL DEFAULT 'in_progress',
    updated_at timestamptz,

    CONSTRAINT check_hashed_code_by_method CHECK (
        (method = 'oauth' AND hashed_code IS NULL) OR
        (method = 'code' AND status = 'in_progress' AND hashed_code IS NOT NULL) OR
        (method = 'code' AND status <> 'in_progress' AND hashed_code IS NULL)
    )
);

CREATE TABLE members (
    discord_user_id bigint PRIMARY KEY NOT NULL,
    joined_at timestamptz NOT NULL,
    name varchar(35) NOT NULL,
    invited_by bigint,
    updated_at timestamptz
);
