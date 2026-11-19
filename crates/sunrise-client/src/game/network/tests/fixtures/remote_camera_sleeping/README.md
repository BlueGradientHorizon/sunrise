Witness for a spectated sleeping living entity (Minecraft 26.3).

Produced with `tools/record_remote_camera_sleeping.py` on the real Minecraft 26.3 client jar: VanillaCameraOracle runs the real `Camera.tick` and `Camera.alignWithEntity` (first person) with a real `Villager` and a real `RemotePlayer` (allocated without a constructor, own synched data and attributes) asleep through the real SLEEPING_POS_ID data in a real red bed block at each of the four facings (`bed: [x, y, z, Direction.get2DDataValue]`: 0 south, 1 west, 2 north, 3 east).
cases.jsonl is the input (floats and doubles as hex bits), vanilla.txt the output.
