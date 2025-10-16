DROP VIEW current_realities;

CREATE VIEW current_realities (player, reality)
  AS SELECT DISTINCT claims.player, tracked_worlds.reality FROM claims
  LEFT JOIN tracked_slots ON claims.slot = tracked_slots.id
  LEFT JOIN tracked_worlds ON tracked_slots.world = tracked_worlds.id
  WHERE (free = 0 OR free IS NULL) AND (status < 2 OR status IS NULL) AND tracked_worlds.reality IS NOT NULL
