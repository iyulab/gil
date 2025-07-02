import shutil
import os

source_path = 'D:/data/gil/gil-py/gil_py/workflow/yaml_parser.py'
dest_path = 'D:/data/gil/gil-py/gil_py/yaml_parser.py'

if os.path.exists(source_path):
    shutil.move(source_path, dest_path)
    print(f"Moved: {source_path} to {dest_path}")
else:
    print(f"Source file not found: {source_path}")
