CREATE TABLE members (
    discord_user_id     BIGINT NOT NULL,
    joined_at           TIMESTAMPTZ NOT NULL,
    
    name                VARCHAR(50) NOT NULL,
    invited_by          BIGINT,
    updated_at          TIMESTAMPTZ NOT NULL DEFAULT now(),

    PRIMARY KEY (discord_user_id),
    FOREIGN KEY (invited_by)
        REFERENCES members(discord_user_id)
        ON DELETE SET NULL,
        
    CONSTRAINT members_should_not_invite_themselves
        CHECK (invited_by IS NULL OR invited_by != discord_user_id)
);

CREATE TABLE contributors (
    member_id       BIGINT NOT NULL,
    joined_at       TIMESTAMPTZ NOT NULL,
    updated_at      TIMESTAMPTZ,

    PRIMARY KEY (member_id),
    FOREIGN KEY (member_id)
        REFERENCES members(discord_user_id)
        ON DELETE CASCADE
);

CREATE TABLE staff (
    member_id   BIGINT NOT NULL,
    joined_at   TIMESTAMPTZ NOT NULL,
    updated_at  TIMESTAMPTZ,
    
    admin       BOOLEAN NOT NULL DEFAULT false,

    PRIMARY KEY (member_id),
    FOREIGN KEY (member_id)
        REFERENCES members(discord_user_id)
        ON DELETE CASCADE
);

CREATE TYPE mc_edition AS ENUM ('java', 'bedrock');

CREATE TABLE linked_mc_accounts (
    member_id   BIGINT NOT NULL,
    uuid        UUID NOT NULL UNIQUE,

    linked_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
    username    VARCHAR(20) NOT NULL,
    edition     mc_edition NOT NULL,

    PRIMARY KEY (member_id, uuid),
    FOREIGN KEY (member_id)
        REFERENCES members (discord_user_id)
        ON DELETE CASCADE
);

CREATE TABLE mc_login_events (
    id              UUID NOT NULL UNIQUE,
    player_uuid     UUID NOT NULL,

    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    ip_address      INET NOT NULL,

    username        VARCHAR(20),
    edition         mc_edition NOT NULL,
    member_id       BIGINT,    

    PRIMARY KEY (id, player_uuid),
    FOREIGN KEY (member_id)
        REFERENCES members(discord_user_id)
        ON DELETE SET NULL
);


CREATE VIEW member_with_flags AS
SELECT
    m.discord_user_id,
    (
          (COALESCE((c.member_id IS NOT NULL)::INT, 0) << 0) -- 1st bit: contributor
        | (COALESCE((s.member_id IS NOT NULL)::INT, 0) << 1) -- 2nd bit: staff
        | (COALESCE((s.admin = TRUE)::INT, 0) << 2) -- 3rd bit: admin
    ) AS flags
FROM members m
LEFT JOIN contributors c ON c.member_id = m.discord_user_id
LEFT JOIN staff s ON s.member_id = m.discord_user_id;

CREATE VIEW member_view AS
SELECT
    m.discord_user_id,
    m.joined_at,
    m.name,
    mf.flags AS flags,
    inviter.discord_user_id AS invited_by,
    inviter.name            AS inviter_name,
    inviter_flags.flags     AS inviter_flags
FROM members m
INNER JOIN member_with_flags mf ON (mf.discord_user_id = m.discord_user_id)
LEFT JOIN members inviter ON (inviter.discord_user_id = m.invited_by)
LEFT JOIN member_with_flags inviter_flags ON (inviter_flags.discord_user_id = m.invited_by);

CREATE VIEW linked_mc_account_view AS
SELECT
    m.*,
    lma.uuid,
    lma.linked_at,
    lma.username,
    lma.edition,
    last_login.created_at AS last_login_at
FROM linked_mc_accounts lma
INNER JOIN member_view m ON m.discord_user_id = lma.member_id
LEFT JOIN (
    SELECT member_id, MAX(created_at) AS created_at
    FROM mc_login_events
    GROUP BY member_id
) last_login ON last_login.member_id = lma.member_id;
