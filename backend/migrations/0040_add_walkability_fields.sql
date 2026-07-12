-- Walk time (minutes) to nearby amenities, editable by the user.
ALTER TABLE listings ADD COLUMN community_center_walk_min INTEGER;
ALTER TABLE listings ADD COLUMN library_walk_min           INTEGER;
