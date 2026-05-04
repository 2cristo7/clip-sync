ALTER TABLE devices ADD COLUMN policy TEXT NOT NULL DEFAULT '{"mode":"read_write"}';
