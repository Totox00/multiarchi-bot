CREATE TABLE realities (
  id INTEGER PRIMARY KEY,
  name TEXT NOT NULL,
  max_claims INTEGER NOT NULL
) STRICT;

ALTER TABLE worlds ADD COLUMN reality INTEGER REFERENCES realities(id);
ALTER TABLE tracked_worlds ADD COLUMN reality INTEGER REFERENCES realities(id);
ALTER TABLE players DROP COLUMN claims;

DROP VIEW worlds_overview;
DROP VIEW preclaims_overview;
DROP VIEW unclaimed_slots;
DROP VIEW current_claims;

CREATE VIEW worlds_overview (id, name, reality, unclaimed, unstarted, in_progress, goal, all_checks, done) AS
  SELECT tracked_worlds.id, tracked_worlds.name, realities.name,
  COUNT(*) FILTER (WHERE claims.player IS NULL),
  COUNT(*) FILTER (WHERE claims.player IS NOT NULL AND status = 0),
  COUNT(*) FILTER (WHERE claims.player IS NOT NULL AND status = 1),
  COUNT(*) FILTER (WHERE claims.player IS NOT NULL AND status = 2),
  COUNT(*) FILTER (WHERE claims.player IS NOT NULL AND status = 3),
  COUNT(*) FILTER (WHERE claims.player IS NOT NULL AND status = 4)
  FROM tracked_worlds INNER JOIN tracked_slots ON tracked_slots.world = tracked_worlds.id
  LEFT JOIN claims ON claims.slot = tracked_slots.id
  LEFT JOIN realities ON realities.id = tracked_worlds.reality
  GROUP BY tracked_worlds.name
  ORDER BY tracked_worlds.id;

CREATE VIEW preclaims_overview (id, name, reality, slots, preclaims) AS
  SELECT worlds.id, worlds.name, realities.name,
  COUNT(DISTINCT slots.id),
  COUNT(preclaims.player)
  FROM worlds INNER JOIN slots ON slots.world = worlds.id
  LEFT JOIN preclaims ON preclaims.slot = slots.id
  LEFT JOIN realities ON realities.id = worlds.reality
  WHERE resolved_preclaims = 0
  GROUP BY worlds.name
  ORDER BY worlds.id;

CREATE VIEW unclaimed_slots (world, reality, slot, games, free)
  AS SELECT tracked_worlds.name, realities.name, tracked_slots.name, tracked_slots.games, tracked_slots.free
  FROM tracked_slots INNER JOIN tracked_worlds ON tracked_slots.world = tracked_worlds.id
  LEFT JOIN realities ON realities.id = tracked_worlds.reality
  WHERE tracked_slots.id NOT IN (SELECT slot FROM claims) ORDER BY tracked_worlds.id DESC;

CREATE VIEW current_realities (player, realities)
  AS SELECT claims.player, COUNT(DISTINCT tracked_worlds.reality) FROM claims
  LEFT JOIN tracked_slots ON claims.slot = tracked_slots.id
  LEFT JOIN tracked_worlds ON tracked_slots.world = tracked_worlds.id
  WHERE (free = 0 OR free IS NULL) AND (status < 2 OR status IS NULL) AND tracked_worlds.reality IS NOT NULL
  GROUP BY claims.player;

CREATE VIEW current_claims (player, reality, claims)
  AS SELECT claims.player, tracked_worlds.reality, COUNT(*) FROM claims
  LEFT JOIN tracked_slots ON claims.slot = tracked_slots.id
  LEFT JOIN tracked_worlds ON tracked_slots.world = tracked_worlds.id
  WHERE (free = 0 OR free IS NULL) AND (status < 2 OR status IS NULL)
  GROUP BY claims.player, tracked_worlds.reality;
