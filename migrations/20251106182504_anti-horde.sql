-- Add migration script here
ALTER TABLE players ADD COLUMN unspent_points INT NOT NULL DEFAULT 0;
ALTER TABLE preclaims ADD COLUMN blocked_by_unspent INT NOT NULL DEFAULT 0;
