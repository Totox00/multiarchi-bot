ALTER TABLE players ADD COLUMN transfer_to INT REFERENCES players(id);
