import fs from 'fs';
import path from 'path';
import ReactMarkdown from 'react-markdown';
import remarkGfm from 'remark-gfm';

interface DocPageProps {
  params: {
    slug: string[];
  };
}

export default async function DocPage({ params }: DocPageProps) {
  const docsPath = path.join(process.cwd(), '..', 'docs');
  const slug = params.slug || ['README']; // Default to README.md if no slug
  const filePath = path.join(docsPath, `${slug.join('/')}.md`);

  let content: string;
  try {
    content = fs.readFileSync(filePath, 'utf-8');
  } catch (error) {
    return (
      <div className="container mx-auto p-8">
        <h1 className="text-4xl font-bold mb-4">404 - Document Not Found</h1>
        <p>The document you are looking for does not exist.</p>
      </div>
    );
  }

  return (
    <div className="container mx-auto p-8 prose dark:prose-invert">
      <ReactMarkdown remarkPlugins={[remarkGfm]}>{content}</ReactMarkdown>
    </div>
  );
}

export async function generateStaticParams() {
  const docsPath = path.join(process.cwd(), '..', 'docs');
  const files: string[] = [];

  function walkSync(currentPath: string) {
    fs.readdirSync(currentPath).forEach((name) => {
      const filePath = path.join(currentPath, name);
      const stat = fs.statSync(filePath);
      if (stat.isFile() && filePath.endsWith('.md')) {
        files.push(path.relative(docsPath, filePath).replace(/\.md$/, ''));
      } else if (stat.isDirectory()) {
        walkSync(filePath);
      }
    });
  }

  walkSync(docsPath);

  return files.map((file) => ({
    slug: file.split('/'),
  }));
}
