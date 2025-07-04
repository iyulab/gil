# Gil-Flow YAML Specification (v1.0)

**Gil-Flow YAML** is the standard for defining language-neutral, node-based workflows.

> **Note**: For a full overview, see [README.md](../README.md). For node specifications, see [NODE_SPEC.md](NODE_SPEC.md).

## Basic Structure

```yaml
version: "1.0"
name: "Workflow Name"
description: "A description of the workflow."

nodes:
  node_id:
    type: "NodeType"
    config: {}
    inputs: {}
    outputs: {}
    conditions: []

flow:
  - node_id
  - [parallel_1, parallel_2]

outputs:
  result: "@node_id.output"
```

## Required Fields

| Field       | Description                   | Example                |
|-------------|-------------------------------|------------------------|
| `version`   | The Gil-Flow standard version. | `"1.0"`                |
| `name`      | A unique identifier for the workflow. | `"Data Processing"`    |
| `nodes`     | The list of node definitions. | (See below)            |
| `flow`      | The execution order of the nodes. | `["step1", "step2"]`   |

## Node Definition

### Basic Structure
```yaml
node_id:
  type: "NodeType"               # Required: The type of the node.
  config: {}                     # Optional: Configuration for the node.
  inputs: {}                     # Optional: Input data for the node.
  outputs: {}                    # Optional: Output mapping for the node.
```

### Key Node Types
- **DataReadFile**: Reads a file.
- **DataTransform**: Transforms data.
- **OpenAIGenerateText**: Generates text using AI.
- **OpenAIGenerateImage**: Generates an image using AI.
- **GilConnectorOpenAI**: A connector for OpenAI.
- **ControlBranch**: Executes a branch based on a condition.
- **UtilLogMessage**: Logs a message.
- **UtilSetVariable**: Sets a variable.

## Reference System

### Environment Variables
```yaml
config:
  api_key: "${OPENAI_API_KEY}"   # Reference an environment variable.
```

### Node Outputs
```yaml
inputs:
  data: "@previous_node.output"  # Reference the output of a previous node.
```

### Context References
The context is used to pass data between nodes via the `Context` object. Values in the context can be referenced using the `${key}` syntax.

```yaml
inputs:
  user_id: "${user_data.id}"
  api_key: "${env.OPENAI_API_KEY}"
```

## Execution Flow

### Sequential Execution
```yaml
flow:
  - step1
  - step2
  - step3
```

### Parallel Execution
```yaml
flow:
  - init
  - [parallel_1, parallel_2, parallel_3]
  - merge
```

### Conditional Flow
```yaml
flow:
  - validator
  - condition:
      if: "@validator.is_valid"
      then: [process_valid]
      else: [handle_error]
```

## Output Definition

### Simple Output
```yaml
outputs:
  result: "@final_node.output"
```

### Composite Output
```yaml
outputs:
  processed_data: "@transform.output_data"
  status: "@validator.status"
```

## Advanced Features

### Retry Settings
```yaml
node_id:
  type: "CommAPI"
  retry:
    max_attempts: 3
    delay: 1000
    backoff: "exponential"
```

### Timeout Settings
```yaml
node_id:
  type: "AITextGen"
  timeout: 30000                 # 30 seconds
```

### Metadata (Optional)
```yaml
metadata:
  author: "Author Name"
  version: "1.0.0"
  tags: ["data", "ai"]
```

## Example Workflows

### Image Generation
```yaml
version: "1.0"
name: "AI Image Generation"

nodes:
  openai_connection:
    type: "GilConnectorOpenAI"
    config:
      api_key: "${OPENAI_API_KEY}"
  
  image_generator:
    type: "OpenAIGenerateImage"
    inputs:
      client: "@openai_connection.client"
      prompt: "A beautiful sunset"
      size: "1024x1024"

flow:
  - openai_connection
  - image_generator

outputs:
  image: "@image_generator.image_url"
```

### Data Pipeline
```yaml
version: "1.0"
name: "Data Processing Pipeline"

nodes:
  file_reader:
    type: "DataReadFile"
    inputs:
      file_path: "data.txt"
  
  data_transformer:
    type: "DataTransform"
    config:
      transform_expression: "data.upper()"
    inputs:
      input_data: "@file_reader.content"
  
  logger:
    type: "UtilLogMessage"
    inputs:
      input: "@data_transformer.output_data"

flow:
  - file_reader
  - data_transformer
  - logger

outputs:
  final_data: "@data_transformer.output_data"
```

---

*For detailed node interfaces, please see [NODE_SPEC.md](NODE_SPEC.md).*
