DROP VIEW current_claims;

CREATE VIEW current_claims (player, claims)
  AS SELECT claims.player, COUNT(*) FROM claims LEFT JOIN tracked_slots ON claims.slot = tracked_slots.id WHERE (free = 0 OR free IS NULL) AND (status < 2 OR status IS NULL) GROUP BY claims.player;