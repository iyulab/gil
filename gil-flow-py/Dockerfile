FROM python:3.9-slim-buster

WORKDIR /app

COPY requirements.txt .
RUN pip install --no-cache-dir -r requirements.txt

COPY . .
COPY ../gil-py /app/gil-py

CMD ["uvicorn", "gil_flow_py.main:app", "--host", "0.0.0.0", "--port", "8000"]
