# Use a lightweight Python image
FROM python:3.10-slim-buster

# Set working directory
WORKDIR /app

# Copy pyproject.toml and poetry.lock (if using poetry) or requirements.txt
# For setuptools, copy pyproject.toml and install build dependencies
COPY gil-flow-py/pyproject.toml ./pyproject.toml

# Install build dependencies and then the project itself
RUN pip install --no-cache-dir poetry && \
    poetry install --no-root --no-dev

# Copy the application code
COPY gil-flow-py/gil_flow_py ./gil_flow_py

# Expose the port FastAPI runs on
EXPOSE 8000

# Command to run the application
CMD ["poetry", "run", "uvicorn", "gil_flow_py.main:app", "--host", "0.0.0.0", "--port", "8000"]