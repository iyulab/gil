

import sys
import os

# Add the gil-py project root to sys.path
sys.path.insert(0, os.path.abspath(os.path.join(os.path.dirname(__file__), '..', '..', 'gil-py')))

import pytest
from fastapi.testclient import TestClient
from gil_flow_py.main import app, get_api_key, get_node_factory_dependency
from gil_py.workflow.node_factory import NodeFactory
from gil_py.core.util.log_message import UtilLogMessageNode
from gil_py.core.util.set_variable import UtilSetVariableNode
from gil_py.core.control.branch import ControlBranchNode

@pytest.fixture(autouse=True)
def override_api_key():
    # Store the original dependency
    original_get_api_key = get_api_key
    # Override the dependency for testing
    app.dependency_overrides[get_api_key] = lambda: "test_api_key"
    yield
    # Restore the original dependency after the test
    app.dependency_overrides[get_api_key] = original_get_api_key

@pytest.fixture
def client(override_api_key):
    test_node_factory = NodeFactory()
    test_node_factory.register("Util-LogMessage", UtilLogMessageNode)
    test_node_factory.register("Util-SetVariable", UtilSetVariableNode)
    test_node_factory.register("Control-Branch", ControlBranchNode)

    app.dependency_overrides[get_node_factory_dependency] = lambda: test_node_factory

    client = TestClient(app)
    yield client

    del app.dependency_overrides[get_node_factory_dependency]

@pytest.mark.asyncio
async def test_health_check(client):
    response = client.get("/health")
    assert response.status_code == 200
    assert response.json() == {"status": "ok"}

@pytest.mark.asyncio
async def test_run_workflow_invalid_api_key(client):
    response = client.post(
        "/workflows/run",
        headers={"X-API-Key": "wrong_key"},
        json={"workflow_yaml": ""}
    )
    assert response.status_code == 401
    assert response.json() == {"detail": "Invalid API Key"}

@pytest.mark.asyncio
async def test_run_workflow_success(client):
    workflow_yaml = """
    name: Main Test Workflow
    nodes:
      main_log_node:
        type: Util-LogMessage
        inputs:
          input: Hello from main test!
    flow:
      - main_log_node
    """
    response = client.post(
        "/workflows/run",
        headers={"X-API-Key": "test_api_key"},
        json={"workflow_yaml": workflow_yaml}
    )
    assert response.status_code == 200
    assert response.json()["status"] == "success"
