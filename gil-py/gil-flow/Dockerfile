# Use a lightweight Python image
FROM python:3.10-slim-buster as builder

# Set working directory
WORKDIR /app

# Copy only requirements.txt to leverage Docker cache
COPY requirements.txt .

# Install build dependencies and then the project itself
RUN pip install --no-cache-dir -r requirements.txt

# Copy gil-py (gil-flow) and gil-node-* packages from correct paths
COPY ../.. /app/gil-py
COPY ../../nodes/gil-node-data /app/gil-node-data
COPY ../../nodes/gil-node-text /app/gil-node-text
COPY ../../nodes/gil-node-openai /app/gil-node-openai

# Reinstall editable packages to ensure they are linked correctly within the container
RUN pip install --no-cache-dir -e /app/gil-py \
    -e /app/gil-node-data \
    -e /app/gil-node-text \
    -e /app/gil-node-openai

# Copy the application code
COPY gil_flow_py ./gil_flow_py

# --- Final Stage ---
FROM python:3.10-slim-buster

WORKDIR /app

# Copy only the installed packages from the builder stage
COPY --from=builder /usr/local/lib/python3.10/site-packages /usr/local/lib/python3.10/site-packages

# Copy the application code
COPY --from=builder /app/gil_flow_py ./gil_flow_py

# Expose the port FastAPI runs on
EXPOSE 8000

# Command to run the application
CMD ["uvicorn", "gil_flow_py.main:app", "--host", "0.0.0.0", "--port", "8000"]
