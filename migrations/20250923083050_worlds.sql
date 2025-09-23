CREATE VIEW worlds_overview (id, name, unclaimed, in_progress, goal, all_checks, done) AS
  SELECT tracked_worlds.id, tracked_worlds.name,
  COUNT(*) FILTER (WHERE status = 0),
  COUNT(*) FILTER (WHERE status = 1),
  COUNT(*) FILTER (WHERE status = 2),
  COUNT(*) FILTER (WHERE status = 3),
  COUNT(*) FILTER (WHERE status = 4)
  FROM tracked_worlds INNER JOIN tracked_slots ON tracked_slots.world = tracked_worlds.id
  GROUP BY tracked_worlds.name
  ORDER BY tracked_worlds.id;
