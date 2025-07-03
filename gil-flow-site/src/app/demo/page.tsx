use client";

import { useState } from "react";

export default function DemoPage() {
  const [workflowYaml, setWorkflowYaml] = useState(
    `name: My First Workflow\nnodes:\n  log_message:\n    type: Util-LogMessage\n    inputs:\n      input: Hello from the demo!`
  );
  const [apiResponse, setApiResponse] = useState<any>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const API_BASE_URL = process.env.NEXT_PUBLIC_GIL_FLOW_API_URL || "http://localhost:8000";
  const API_KEY = process.env.NEXT_PUBLIC_GIL_FLOW_API_KEY || "test_api_key";

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    setLoading(true);
    setApiResponse(null);
    setError(null);

    try {
      const response = await fetch(`${API_BASE_URL}/workflows/run`, {
        method: "POST",
        headers: {
          "Content-Type": "application/json",
          "X-API-Key": API_KEY,
        },
        body: JSON.stringify({
          workflow_yaml: workflowYaml,
          context: {},
        }),
      });

      const data = await response.json();

      if (!response.ok) {
        throw new Error(data.detail || "Something went wrong");
      }

      setApiResponse(data);
    } catch (err: any) {
      setError(err.message);
    } finally {
      setLoading(false);
    }
  };

  return (
    <div className="min-h-screen bg-gray-100 dark:bg-gray-900 text-gray-900 dark:text-gray-100 p-4">
      <main className="container mx-auto py-8">
        <h1 className="text-4xl font-bold text-center mb-8">Gil-Flow Demo</h1>

        <form onSubmit={handleSubmit} className="bg-white dark:bg-gray-800 p-6 rounded-lg shadow-md mb-8">
          <div className="mb-4">
            <label htmlFor="workflowYaml" className="block text-lg font-medium mb-2">
              Workflow YAML
            </label>
            <textarea
              id="workflowYaml"
              className="w-full p-3 border border-gray-300 dark:border-gray-700 rounded-md bg-gray-50 dark:bg-gray-900 text-gray-900 dark:text-gray-100 font-mono"
              rows={15}
              value={workflowYaml}
              onChange={(e) => setWorkflowYaml(e.target.value)}
              placeholder="Enter your Gil-Flow YAML here..."
            ></textarea>
          </div>
          <button
            type="submit"
            className="w-full bg-blue-600 text-white py-3 rounded-lg font-semibold text-lg hover:bg-blue-700 transition-colors disabled:opacity-50"
            disabled={loading}
          >
            {loading ? "Running Workflow..." : "Run Workflow"}
          </button>
        </form>

        {error && (
          <div className="bg-red-100 dark:bg-red-900 border border-red-400 text-red-700 dark:text-red-300 px-4 py-3 rounded relative mb-8" role="alert">
            <strong className="font-bold">Error:</strong>
            <span className="block sm:inline"> {error}</span>
          </div>
        )}

        {apiResponse && (
          <div className="bg-white dark:bg-gray-800 p-6 rounded-lg shadow-md">
            <h2 className="text-2xl font-semibold mb-4">API Response</h2>
            <pre className="bg-gray-50 dark:bg-gray-900 p-4 rounded-md overflow-x-auto font-mono text-sm">
              {JSON.stringify(apiResponse, null, 2)}
            </pre>
          </div>
        )}
      </main>
    </div>
  );
}