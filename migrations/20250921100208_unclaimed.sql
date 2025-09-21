CREATE VIEW unclaimed_slots (world, slot, games, free)
  AS SELECT tracked_worlds.name, tracked_slots.name, tracked_slots.games, tracked_slots.free
  FROM tracked_slots INNER JOIN tracked_worlds ON tracked_slots.world = tracked_worlds.id
  WHERE tracked_slots.id NOT IN (SELECT slot FROM claims) ORDER BY tracked_worlds.id DESC
  