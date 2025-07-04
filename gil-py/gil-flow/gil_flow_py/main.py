from fastapi import FastAPI, HTTPException, Header, Depends
from pydantic import BaseModel
from typing import Dict, Any
import os

# gil-py imports
from gil_py.workflow.executor import WorkflowExecutor
from gil_py.core.context import Context
from gil_py.workflow.node_factory import NodeFactory
import yaml
from gil_py.yaml_parser import YamlWorkflowParser as YamlParser

app = FastAPI()

# Initialize NodeFactory globally to discover nodes once
_node_factory_instance = NodeFactory()

# Dependency to get NodeFactory instance
def get_node_factory_dependency():
    return _node_factory_instance

# In a real application, API keys would be stored securely (e.g., database)
# and managed with proper authentication mechanisms.
def get_api_key():
    return os.getenv("GIL_FLOW_API_KEY", "test_api_key")

class WorkflowRequest(BaseModel):
    workflow_yaml: str
    context: Dict[str, Any] = {}

@app.get("/health")
async def health_check():
    return {"status": "ok"}

@app.get("/nodes")
async def get_available_nodes(node_factory: NodeFactory = Depends(get_node_factory_dependency)):
    """Returns a list of all available node types discovered by the NodeFactory."""
    return {"available_nodes": node_factory.get_available_nodes()}

@app.post("/workflows/run")
async def run_workflow(request: WorkflowRequest, x_api_key: str = Header(...), node_factory: NodeFactory = Depends(get_node_factory_dependency)):
    if x_api_key != get_api_key():
        raise HTTPException(status_code=401, detail="Invalid API Key")

    try:
        # Parse the workflow YAML
        parser = YamlParser()
        workflow_dict = yaml.safe_load(request.workflow_yaml)
        workflow = parser.parse_dict(workflow_dict)

        # Create context
        context = Context(request.context)

        # Execute the workflow
        executor = WorkflowExecutor(node_factory=node_factory)
        result = await executor.execute(workflow, context)

        return {"status": "success", "result": result}
    except Exception as e:
        raise HTTPException(status_code=500, detail=str(e))

if __name__ == "__main__":
    import uvicorn
    uvicorn.run(app, host="0.0.0.0", port=8000)