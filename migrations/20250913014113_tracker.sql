CREATE TABLE players (
  id INTEGER PRIMARY KEY,
  snowflake INTEGER NOT NULL,
  claims INTEGER NOT NULL
) STRICT;

CREATE TABLE tracked_worlds (
  id INTEGER PRIMARY KEY,
  tracker_id TEXT UNIQUE NOT NULL,
  name TEXT UNIQUE NOT NULL,
  last_scrape INTEGER NOT NULL DEFAULT (strftime('%s', 'now')),
  done INTEGER NOT NULL DEFAULT 0
) STRICT;

CREATE TABLE tracked_slots (
  id INTEGER PRIMARY KEY,
  world INTEGER NOT NULL REFERENCES tracked_worlds(id),
  name TEXT NOT NULL,
  games TEXT NOT NULL,
  -- 0 = unstarted
  -- 1 = in progress
  -- 2 = goal
  -- 3 = all checks
  -- 4 = done
  status INTEGER NOT NULL,
  checks INTEGER NOT NULL,
  checks_total INTEGER NOT NULL,
  last_activity INTEGER
  free INTEGER NOT NULL DEFAULT 0
) STRICT;

CREATE TABLE claims (
  id INTEGER PRIMARY KEY,
  slot INTEGER NOT NULL REFERENCES tracked_slots(id),
  player INTEGER NOT NULL REFERENCES players(id),
) STRICT;

CREATE TABLE updates (
  id INTEGER PRIMARY KEY,
  slot INTEGER NOT NULL REFERENCES tracked_slots(id),
  timestamp INTEGER NOT NULL DEFAULT (strftime('%s', 'now')),
  player INTEGER NOT NULL REFERENCES players(id),
  description TEXT NOT NULL
) STRICT;

CREATE VIEW current_claims (player, claims)
  AS SELECT claims.player, COUNT(*) FROM claims LEFT JOIN tracked_slots ON claims.slot = tracked_slots.id WHERE free = 0 OR free is NULL GROUP BY claims.player;

DROP TABLE preclaims;

CREATE TABLE preclaims (
  slot INTEGER NOT NULL REFERENCES slots(id),
  player INTEGER NOT NULL REFERENCES players(id),
  -- 0 = unresolved
  -- 1 = unselected
  -- 2 = selected
  status INTEGER NOT NULL DEFAULT 0
) STRICT;
