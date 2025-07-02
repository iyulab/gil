import pytest
from fastapi.testclient import TestClient
from gil_flow_py.main import app, get_api_key

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
    return TestClient(app)

def test_health_check(client):
    response = client.get("/health")
    assert response.status_code == 200
    assert response.json() == {"status": "ok"}

def test_get_nodes(client):
    response = client.get("/nodes")
    assert response.status_code == 200
    # Check if some expected core nodes are present
    assert "available_nodes" in response.json()
    assert "Util-LogMessage" in response.json()["available_nodes"]
    assert "Util-SetVariable" in response.json()["available_nodes"]
    assert "Control-Branch" in response.json()["available_nodes"]

def test_run_workflow_success(client):
    workflow_yaml = """
    nodes:
      - id: my_log_node
        type: Util-LogMessage
        config:
          prefix: TestWorkflow
        inputs:
          input: Hello from test!
    """
    response = client.post(
        "/workflows/run",
        headers={"X-API-Key": "test_api_key"},
        json={
            "workflow_yaml": workflow_yaml,
            "context": {"test_var": "initial"}
        }
    )
    assert response.status_code == 200
    assert response.json()["status"] == "success"
    assert "my_log_node" in response.json()["result"]["node_outputs"]
    assert response.json()["result"]["node_outputs"]["my_log_node"]["output"] == "Hello from test!"
    assert response.json()["result"]["context"]["test_var"] == "initial"

def test_run_workflow_invalid_api_key(client):
    workflow_yaml = """
    nodes:
      - id: my_log_node
        type: Util-LogMessage
        config:
          prefix: TestWorkflow
        inputs:
          input: Hello from test!
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

def test_run_workflow_invalid_yaml(client):
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
    assert "Error parsing YAML" in response.json()["detail"]
