import os
from datetime import datetime

# Define the source directory
source_path = "./packagejson"

# Loop through all items in the source directory
for dir_name in os.listdir(source_path):
    old_dir_path = os.path.join(source_path, dir_name)
    
    if os.path.isdir(old_dir_path):
      if dir_name.startswith("test_"):

        new_dir_path = os.path.join(source_path, dir_name[5:])
        os.rename(old_dir_path, new_dir_path)
