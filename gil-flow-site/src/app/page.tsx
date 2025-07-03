import Link from "next/link";

export default function Home() {
  return (
    <div className="min-h-screen flex flex-col items-center justify-center bg-gray-100 dark:bg-gray-900 text-gray-900 dark:text-gray-100 p-4">
      <main className="flex flex-col items-center text-center gap-8 max-w-4xl mx-auto">
        <h1 className="text-5xl font-bold tracking-tight sm:text-6xl">
          Gil-Flow: Your Workflow Automation Engine
        </h1>
        <p className="text-xl text-gray-700 dark:text-gray-300 leading-relaxed">
          Gil-Flow is a powerful, language-neutral, and YAML-based workflow system designed for seamless automation. Build complex pipelines with ease, integrate AI services, and manage your data flows efficiently.
        </p>

        <div className="flex flex-col sm:flex-row gap-4 mt-6">
          <Link href="/docs" className="px-6 py-3 bg-blue-600 text-white rounded-lg shadow-md hover:bg-blue-700 transition-colors text-lg font-medium">
            Read the Docs
          </Link>
          <Link href="/demo" className="px-6 py-3 bg-green-600 text-white rounded-lg shadow-md hover:bg-green-700 transition-colors text-lg font-medium">
            Try the Demo
          </Link>
        </div>

        <section className="mt-12 w-full text-left">
          <h2 className="text-3xl font-semibold mb-4">Get Started</h2>
          <div className="bg-white dark:bg-gray-800 p-6 rounded-lg shadow-md">
            <p className="mb-4">To get started with Gil-Flow, you can install the core SDK and node packages via pip:</p>
            <pre className="bg-gray-200 dark:bg-gray-700 text-gray-800 dark:text-gray-200 p-4 rounded-md overflow-x-auto">
              <code className="language-bash">
                pip install gil-py gil-node-data gil-node-text gil-node-openai
              </code>
            </pre>
            <p className="mt-4">For more detailed installation and usage instructions, please refer to our documentation.</p>
          </div>
        </section>
      </main>

      <footer className="mt-12 text-gray-600 dark:text-gray-400 text-sm">
        © {new Date().getFullYear()} Gil-Flow. All rights reserved.
      </footer>
    </div>
  );
}