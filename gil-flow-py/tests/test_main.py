

import pytest
from fastapi.testclient import TestClient
from gil_flow_py.main import app

client = TestClient(app)

def test_health_check():
    response = client.get("/health")
    assert response.status_code == 200
    assert response.json() == {"status": "ok"}

def test_run_workflow_invalid_api_key():
    response = client.post(
        "/workflows/run",
        headers={"X-API-Key": "wrong_key"},
        json={"workflow_yaml": ""}
    )
    assert response.status_code == 401
    assert response.json() == {"detail": "Invalid API Key"}

def test_run_workflow_success():
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
        headers={"X-API-Key": "your_super_secret_api_key"},
        json={"workflow_yaml": workflow_yaml}
    )
    assert response.status_code == 200
    assert response.json()["status"] == "success"
