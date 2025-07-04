# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Gil is a language-neutral, node-based workflow standard that enables complex workflow composition through self-contained nodes. This repository contains:

- **Gil-Flow Standard**: Language-neutral YAML-based workflow specification
- **gil-py**: Python implementation of the Gil-Flow standard
- **gil-flow-site**: Next.js documentation website

## Core Architecture

### Language-Neutral Standard
- **Gil-Flow YAML**: Standardized workflow definition format
- **Node Interface**: Common node specifications across all implementations
- **Context System**: Shared data and state management between nodes

### Python Implementation (gil-py)
- **Core System**: Base classes for nodes, ports, connections, and context
- **Workflow Engine**: YAML parser, executor, and node factory
- **CLI Tools**: Command-line interface for workflow execution and validation
- **Node Types**: Built-in core nodes and extensible node packages

## Common Development Commands

### Python (gil-py)
```bash
# Install in development mode
cd gil-py
pip install -e .[dev]

# Run tests
pytest

# Run linting
ruff check .

# Run type checking
mypy gil/

# CLI usage
gil-flow run workflow.yaml
gil-flow validate workflow.yaml
gil-flow list-nodes
```

### Next.js Website (gil-flow-site)
```bash
# Development server
cd gil-flow-site
npm run dev

# Build for production
npm run build

# Start production server
npm start

# Lint
npm run lint
```

## Development Workflow

### Testing Workflows
The `gil-py/tests/` directory contains example workflows:
- `generate-image.yaml`: Simple OpenAI DALL-E image generation
- `smart-content-generator.yaml`: Complex multi-step content generation with context
- `workflows/`: Additional workflow examples

### Adding New Nodes
1. Create node class inheriting from `Node` base class
2. Implement abstract `execute` method
3. Register in `pyproject.toml` entry points under `[project.entry-points."gil.nodes"]`
4. Add corresponding documentation in `docs/nodes/`

### Code Architecture Patterns

#### Node Structure
- All nodes inherit from `gil_py.core.node.Node`
- Nodes are async and use `execute(data, context)` method
- Input/output ports define data flow interfaces
- Context provides shared state and variable resolution

#### Workflow Execution
- `GilWorkflow` class manages workflow lifecycle
- `YamlWorkflowParser` handles YAML parsing and validation
- `WorkflowExecutor` manages node execution order and dependencies
- `NodeFactory` dynamically creates node instances from type strings

#### Context System
- `Context` class provides key-value storage with reference resolution
- Environment variable substitution with `${VAR}` syntax
- Node output references with `@node.output` syntax

## Documentation Structure

- **README.md**: Project overview and quick start
- **docs/YAML_SPEC.md**: Gil-Flow YAML syntax specification
- **docs/NODE_SPEC.md**: Standard node types and interfaces
- **docs/ARCHITECTURE.md**: Language-neutral architecture guide
- **docs/DEV.md**: gil-py implementation development guide
- **docs/CONTEXT_SYSTEM.md**: Context system documentation
- **docs/nodes/**: Individual node documentation

## Key Dependencies

### Python (gil-py)
- `pydantic>=2.0.0`: Data validation and serialization
- `aiohttp>=3.8.0`: Async HTTP client
- `pyyaml>=6.0.0`: YAML parsing
- `python-dotenv>=1.0.0`: Environment variable loading
- `click>=8.0.0`: CLI framework

### Next.js (gil-flow-site)
- `next`: React framework
- `marked`: Markdown processing
- `mermaid`: Diagram rendering
- `tailwindcss`: CSS framework

## File Structure Conventions

```
gil/
├── README.md                 # Project overview
├── docs/                     # Documentation
│   ├── YAML_SPEC.md         # YAML specification
│   ├── NODE_SPEC.md         # Node specifications
│   └── nodes/               # Individual node docs
├── gil-py/                  # Python implementation
│   ├── gil/                 # Core library
│   │   ├── core/           # Base classes
│   │   ├── workflow/       # Workflow engine
│   │   └── cli/            # CLI tools
│   └── tests/              # Test workflows
└── gil-flow-site/          # Documentation website
    └── src/app/            # Next.js app
```

## Quality Standards

- Maintain language neutrality in standard definitions
- Follow async/await patterns for all I/O operations
- Use type hints and Pydantic models for data validation
- Keep node implementations self-contained and dependency-minimal
- Ensure all workflows can be validated before execution
- Write comprehensive tests for new nodes and workflows

## Environment Setup

Required environment variables:
- `OPENAI_API_KEY`: For OpenAI integration nodes
- `OPENAI_ORG`: Optional OpenAI organization ID

The project uses `.env` files for local development configuration.