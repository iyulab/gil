# Gil-Flow-Py API Server

This project provides a RESTful API server for executing and managing Gil-Flow workflows. It allows you to run your `gil-py` workflows via HTTP requests, making it easy to integrate with other applications and services.

## Features

*   Execute Gil-Flow workflows via API.
*   API Key based authentication.
*   Dockerized for easy deployment.

## Getting Started

### Prerequisites

*   Docker and Docker Compose installed.
*   Python 3.8+

### Local Development

1.  **Clone the repository (if you haven't already):**

    ```bash
    git clone https://github.com/your-repo/gil-flow.git
    cd gil-flow
    ```

2.  **Navigate to the `gil-flow-py` directory:**

    ```bash
    cd gil-flow-py
    ```

3.  **Build and run the Docker container:**

    ```bash
    docker-compose up --build
    ```

    The API server will be running at `http://localhost:8000`.

### API Key

The API server uses a simple API Key for authentication. By default, the key is `your_super_secret_api_key`. You can change this by setting the `GIL_FLOW_API_KEY` environment variable in your `docker-compose.yml` or when running the Docker container.

## API Endpoints

### Health Check

`GET /health`

Returns the status of the API server.

**Response:**

```json
{
  "status": "ok"
}
```

### Run Workflow

`POST /workflows/run`

Executes a Gil-Flow workflow defined in YAML.

**Headers:**

*   `X-API-Key`: Your API Key (e.g., `your_super_secret_api_key`)

**Request Body:**

```json
{
  "workflow_yaml": "string",
  "context": {
    "key": "value"
  }
}
```

*   `workflow_yaml`: The Gil-Flow workflow defined in YAML format.
*   `context`: (Optional) A dictionary of context variables to be passed to the workflow.

**Response (Success):**

```json
{
  "status": "success",
  "result": {
    "node_name": "output_value"
  }
}
```

**Response (Error):**

```json
{
  "detail": "Error message"
}
```

## Example Usage (using `curl`)

```bash
curl -X POST http://localhost:8000/workflows/run \
-H "X-API-Key: your_super_secret_api_key" \
-H "Content-Type: application/json" \
-d '{ "workflow_yaml": "---\n  nodes:\n    - id: hello_node\n      type: AITextGeneration\n      config:\n        prompt: \"Hello, Gil-Flow!\"\n", "context": {} }'
```

## Deployment

To deploy this API server, you can build the Docker image and run it on your preferred Docker hosting platform (e.g., AWS ECS, Google Cloud Run, Kubernetes).

```bash
docker build -t gil-flow-py:latest .
docker run -p 8000:8000 -e GIL_FLOW_API_KEY=your_production_api_key gil-flow-py:latest
```
