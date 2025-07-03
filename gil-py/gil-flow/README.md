# Gil-Flow-Py API Host

This project provides a RESTful API for executing and managing Gil-Flow workflows.
It leverages `gil-py` as the core workflow engine and dynamically loads nodes from installed `gil-node-*` packages.

## Features

- **Workflow Execution**: Run Gil-Flow YAML workflows via API.
- **Dynamic Node Discovery**: Automatically discovers and loads nodes from `gil-node-*` packages.
- **Health Check**: Basic health endpoint.
- **API Key Authentication**: Simple API key based authentication.
- **Dockerized**: Easy deployment using Docker.

## Getting Started

### Prerequisites

- Python 3.8+
- Docker (for containerized deployment)

### Local Development

1.  **Clone the repository:**
    ```bash
    git clone https://github.com/iyulab/gil.git
    cd gil
    ```

2.  **Install dependencies (from the `gil-flow-py` directory):**
    ```bash
    cd gil-flow-py
    pip install -e .
    # If you are developing gil-py or gil-node-* packages locally, 
    # ensure they are installed in editable mode as well:
    # pip install -e ../gil-py
    # pip install -e ../gil-node-openai
    # pip install -e ../gil-node-text
    # pip install -e ../gil-node-data
    ```

3.  **Set your API Key:**
    Create a `.env` file in the `gil-flow-py` directory with your API key:
    ```
    GIL_FLOW_API_KEY=your_super_secret_api_key
    ```

4.  **Run the API server:**
    ```bash
    uvicorn gil_flow_py.main:app --host 0.0.0.0 --port 8000
    ```
    The API will be accessible at `http://localhost:8000`.

### Docker Deployment

1.  **Build the Docker image (from the `gil-flow-py` directory):**
    ```bash
    docker build -t gil-flow-api .
    ```

2.  **Run the Docker container:**
    ```bash
    docker run -d -p 8000:8000 --name gil-flow-api-container -e GIL_FLOW_API_KEY=your_super_secret_api_key gil-flow-api
    ```
    Alternatively, use `docker-compose`:
    ```bash
    docker-compose up -d
    ```

## API Endpoints

- **`GET /health`**
    - **Description**: Checks the health of the API server.
    - **Response**: `{"status": "ok"}`

- **`GET /nodes`**
    - **Description**: Returns a list of all dynamically discovered and available node types.
    - **Response**: `{"available_nodes": ["Control-Branch", "OpenAI-Connector", ...]}`

- **`POST /workflows/run`**
    - **Description**: Executes a Gil-Flow workflow defined in YAML.
    - **Headers**:
        - `X-API-Key`: Your API key.
    - **Request Body (JSON)**:
        ```json
        {
            "workflow_yaml": "---\n  nodes:\n    - id: my_log_node\n      type: Util-LogMessage\n      config:\n        prefix: MyWorkflow\n      inputs:\n        input: Hello Gil-Flow!\n",
            "context": {
                "initial_value": "some_data"
            }
        }
        ```
    - **Response (JSON)**:
        ```json
        {
            "status": "success",
            "result": {
                "node_outputs": {
                    "my_log_node": {
                        "output": "Hello Gil-Flow!"
                    }
                },
                "context": {
                    "initial_value": "some_data"
                }
            }
        }
        ```

## Contributing

Refer to the main project's `CONTRIBUTING.md` (if available) and `docs/` for more information on contributing to Gil-Flow and developing custom nodes.