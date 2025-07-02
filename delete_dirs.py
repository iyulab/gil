import shutil
import os

def delete_pycache(path):
    for root, dirs, files in os.walk(path):
        if '__pycache__' in dirs:
            shutil.rmtree(os.path.join(root, '__pycache__'), ignore_errors=True)
            print(f"Deleted: {os.path.join(root, '__pycache__')}")

build_path = 'D:/data/gil/gil-py/build'
egg_info_path = 'D:/data/gil/gil-py/gil_flow.egg-info'

if os.path.exists(build_path):
    shutil.rmtree(build_path, ignore_errors=True)
    print(f"Deleted: {build_path}")

if os.path.exists(egg_info_path):
    shutil.rmtree(egg_info_path, ignore_errors=True)
    print(f"Deleted: {egg_info_path}")

delete_pycache('D:/data/gil/gil-py')
delete_pycache('D:/data/gil/gil-flow-py')