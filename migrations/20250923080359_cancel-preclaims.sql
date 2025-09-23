DROP TABLE preclaims;
DROP TABLE slots;
DROP TABLE worlds;

CREATE TABLE worlds (
  id INTEGER PRIMARY KEY,
  name TEXT UNIQUE NOT NULL,
  preclaim_end INTEGER NOT NULL,
  resolved_preclaims INTEGER NOT NULL DEFAULT 0
) STRICT;

CREATE TABLE slots (
  id INTEGER PRIMARY KEY,
  world INTEGER NOT NULL REFERENCES worlds(id) ON DELETE CASCADE,
  name TEXT NOT NULL,
  games TEXT NOT NULL,
  notes TEXT NOT NULL,
  points TEXT NOT NULL
) STRICT;

CREATE TABLE preclaims (
  slot INTEGER NOT NULL REFERENCES slots(id) ON DELETE CASCADE,
  player INTEGER NOT NULL REFERENCES players(id) ON DELETE CASCADE,
  -- 0 = unresolved
  -- 1 = unselected
  -- 2 = selected
  status INTEGER NOT NULL DEFAULT 0
) STRICT;
