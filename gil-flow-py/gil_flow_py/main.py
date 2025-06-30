from fastapi import FastAPI, HTTPException, Header
from pydantic import BaseModel
from typing import Dict, Any
import os

# gil-py imports
from gil_py.workflow.yaml_parser import YamlParser
from gil_py.workflow.executor import WorkflowExecutor
from gil_py.core.context import Context

app = FastAPI()

# In a real application, API keys would be stored securely (e.g., database)
# and managed with proper authentication mechanisms.
API_KEY = os.getenv("GIL_FLOW_API_KEY", "your_super_secret_api_key")

class WorkflowRequest(BaseModel):
    workflow_yaml: str
    context: Dict[str, Any] = {}

@app.get("/health")
async def health_check():
    return {"status": "ok"}

@app.post("/workflows/run")
async def run_workflow(request: WorkflowRequest, x_api_key: str = Header(...)):
    if x_api_key != API_KEY:
        raise HTTPException(status_code=401, detail="Invalid API Key")

    try:
        # Parse the workflow YAML
        parser = YamlParser()
        workflow = parser.parse(request.workflow_yaml)

        # Create context
        context = Context(request.context)

        # Execute the workflow
        executor = WorkflowExecutor(workflow, context)
        result = await executor.execute()

        return {"status": "success", "result": result.to_dict()}
    except Exception as e:
        raise HTTPException(status_code=500, detail=str(e))

if __name__ == "__main__":
    import uvicorn
    uvicorn.run(app, host="0.0.0.0", port=8000)