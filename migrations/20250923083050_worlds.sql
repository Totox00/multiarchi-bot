CREATE VIEW worlds_overview (id, name, unclaimed, unstarted, in_progress, goal, all_checks, done) AS
  SELECT tracked_worlds.id, tracked_worlds.name,
  COUNT(*) FILTER (WHERE claims.player IS NULL),
  COUNT(*) FILTER (WHERE claims.player IS NOT NULL AND status = 0),
  COUNT(*) FILTER (WHERE claims.player IS NOT NULL AND status = 1),
  COUNT(*) FILTER (WHERE claims.player IS NOT NULL AND status = 2),
  COUNT(*) FILTER (WHERE claims.player IS NOT NULL AND status = 3),
  COUNT(*) FILTER (WHERE claims.player IS NOT NULL AND status = 4)
  FROM tracked_worlds INNER JOIN tracked_slots ON tracked_slots.world = tracked_worlds.id
  LEFT JOIN claims ON claims.slot = tracked_slots.id
  GROUP BY tracked_worlds.name
  ORDER BY tracked_worlds.id;
