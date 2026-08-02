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
    updated_at timestamptz
);
