-- Add migration script here
ALTER TABLE realities ADD COLUMN external INT NOT NULL DEFAULT 0;

CREATE VIEW current_realities_claims (player, realities)
  AS SELECT claims.player, COUNT(DISTINCT tracked_worlds.reality) FROM claims
  LEFT JOIN tracked_slots ON claims.slot = tracked_slots.id
  LEFT JOIN tracked_worlds ON tracked_slots.world = tracked_worlds.id
  LEFT JOIN realities ON realities.id = tracked_worlds.reality
  WHERE (free = 0 OR free IS NULL) AND (status < 2 OR status IS NULL) AND tracked_worlds.reality IS NOT NULL AND external = 0
  GROUP BY claims.player;
  