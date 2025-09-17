ALTER TABLE claims ADD COLUMN public TEXT;

CREATE VIEW public_claims (world, slot, description)
  AS SELECT tracked_worlds.name, tracked_slots.name, public FROM claims
    INNER JOIN tracked_slots ON claims.slot = tracked_slots.id
    INNER JOIN tracked_worlds ON tracked_slots.world = tracked_worlds.id
    WHERE public IS NOT NULL;
