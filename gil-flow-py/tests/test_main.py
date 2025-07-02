

import pytest
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

def test_run_workflow_invalid_api_key(client):
    response = client.post(
        "/workflows/run",
        headers={"X-API-Key": "wrong_key"},
        json={"workflow_yaml": ""}
    )
    assert response.status_code == 401
    assert response.json() == {"detail": "Invalid API Key"}

def test_run_workflow_success(client):
    workflow_yaml = """
    workflow:
      name: Test Workflow
      nodes:
        - name: Start
          type: PassThrough
          connections:
            - target: End
        - name: End
          type: PassThrough
    """
    response = client.post(
        "/workflows/run",
        headers={"X-API-Key": "test_api_key"},
        json={"workflow_yaml": workflow_yaml}
    )
    assert response.status_code == 200
    assert response.json()["status"] == "success"
