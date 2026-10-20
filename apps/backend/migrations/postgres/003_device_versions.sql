ALTER TABLE devices ADD COLUMN ui_version TEXT NOT NULL DEFAULT '';
ALTER TABLE devices ADD COLUMN system_name TEXT NOT NULL DEFAULT '';
ALTER TABLE devices ADD COLUMN system_version TEXT NOT NULL DEFAULT '';

UPDATE devices
SET ui_version = client_version
WHERE ui_version = '' AND client_version LIKE 'future-ui-%';

UPDATE devices
SET client_version = ''
WHERE client_version LIKE 'future-ui-%';
