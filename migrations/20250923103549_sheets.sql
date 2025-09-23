ALTER TABLE players ADD COLUMN name TEXT NOT NULL;

CREATE VIEW sheets_push (world, slot, status, free, player)
  AS SELECT tracked_worlds.name, tracked_slots.name, status, free, players.name FROM tracked_worlds
  INNER JOIN tracked_slots ON tracked_slots.world = tracked_worlds.id
  LEFT JOIN claims ON claims.slot = tracked_slots.id
  LEFT JOIN players ON claims.player = players.id;
