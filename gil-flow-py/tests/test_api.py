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
async def test_get_nodes(client):
    response = client.get("/nodes")
    assert response.status_code == 200
    # Check if some expected core nodes are present
    assert "available_nodes" in response.json()
    assert "Util-LogMessage" in response.json()["available_nodes"]
    assert "Util-SetVariable" in response.json()["available_nodes"]
    assert "Control-Branch" in response.json()["available_nodes"]

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
        json={
            "workflow_yaml": workflow_yaml,
            "context": {}
        }
    )
    assert response.status_code == 200
    assert response.json()["status"] == "success"
    assert "main_log_node" in response.json()["result"]["node_outputs"]
    assert response.json()["result"]["node_outputs"]["main_log_node"]["output"] == "Hello from main test!"
    assert response.json()["result"]["context"] == {}

@pytest.mark.asyncio
async def test_run_workflow_invalid_api_key(client):
    workflow_yaml = """
    name: Test Workflow
    nodes:
      main_log_node:
        type: Util-LogMessage
        config:
          prefix: TestWorkflow
        inputs:
          input: Hello from main test!
    flow:
      - main_log_node
    """
    response = client.post(
        "/workflows/run",
        headers={"X-API-Key": "wrong_api_key"},
        json={
            "workflow_yaml": workflow_yaml,
            "context": {"test_var": "initial"}
        }
    )
    assert response.status_code == 401
    assert response.json()["detail"] == "Invalid API Key"

@pytest.mark.asyncio
async def test_run_workflow_invalid_yaml(client):
    workflow_yaml = """
    invalid_yaml: [
      - item1
    """
    response = client.post(
        "/workflows/run",
        headers={"X-API-Key": "test_api_key"},
        json={
            "workflow_yaml": workflow_yaml,
            "context": {}
        }
    )
    assert response.status_code == 500
    assert "detail" in response.json()
    assert "while parsing a flow node" in response.json()["detail"]
