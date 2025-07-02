import sys
sys.path.append('D:/data/gil/gil-py')

try:
    from gil_py.workflow.yaml_parser import YamlWorkflowParser
    print("Successfully imported YamlWorkflowParser")
except ModuleNotFoundError as e:
    print(f"ModuleNotFoundError: {e}")
except Exception as e:
    print(f"An unexpected error occurred: {e}")
