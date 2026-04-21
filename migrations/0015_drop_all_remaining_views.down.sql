CREATE VIEW member_view AS
SELECT
    member.discord_user_id,
    member.joined_at,
    member.name,
    inviter.discord_user_id as invited_by,
    inviter.name as inviter_name,
    CASE
        WHEN staff.admin = TRUE THEN 'admin'
        WHEN staff.member_id IS NOT NULL THEN 'staff'
        WHEN contributor.member_id IS NOT NULL THEN 'contributor'
        ELSE 'member'
    END AS rank
FROM members member
LEFT JOIN staffs staff
    ON staff.member_id = member.discord_user_id
LEFT JOIN members inviter
    ON (inviter.discord_user_id = member.invited_by)
LEFT JOIN contributors contributor
    ON contributor.member_id = member.discord_user_id;

CREATE VIEW mc_account_view AS
SELECT
    mc_account.discord_user_id as member_id,
    member.name as member_name,
    member.joined_at,
    member.rank as member_rank,
    mc_account.uuid,
    mc_account.username,
    mc_account."type",
    last_session.created_at as "last_login_at"
FROM minecraft_accounts mc_account
LEFT JOIN (
    SELECT member_id, MAX(created_at) as created_at
    FROM logged_in_events
    GROUP BY member_id
) last_session
    ON last_session.member_id = mc_account.discord_user_id
INNER JOIN member_view member
    ON member.discord_user_id = mc_account.discord_user_id;
