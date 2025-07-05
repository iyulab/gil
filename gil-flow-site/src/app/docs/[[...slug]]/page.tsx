import fs from 'fs';
import path from 'path';
// import { marked } from 'marked';
import OptimizedSidebar from '../../components/OptimizedSidebar';
import { getDocTree } from '@/lib/docs';
import MarkdownRenderer from '../../components/MarkdownRenderer'; // New client component

const docsPath = path.join(process.cwd(), '..', 'docs-content');
const docTree = getDocTree(docsPath);

interface DocPageProps {
  params: Promise<{
    slug: string[];
  }>;
}

export default async function DocPage({ params }: DocPageProps) {
  const resolvedParams = await params;
  const slug = resolvedParams.slug && resolvedParams.slug.length > 0 ? resolvedParams.slug : ['OVERVIEW']; // Default to OVERVIEW.md if no slug
  const filePath = path.join(docsPath, `${slug.join('/')}.md`);

  let content: string;
  try {
    content = fs.readFileSync(filePath, 'utf-8');
  } catch {
    return (
      <div className="flex">
        <OptimizedSidebar currentSlug={slug} docTree={docTree} />
        <div className="container mx-auto p-8">
          <h1 className="text-4xl font-bold mb-4">404 - Document Not Found</h1>
          <p>The document you are looking for does not exist.</p>
        </div>
      </div>
    );
  }

  // const rawHtmlContent = marked.parse(content);

  return (
    <div className="flex h-screen">
      <OptimizedSidebar currentSlug={slug} docTree={docTree} />
      <div className="flex-1 overflow-y-auto p-8 prose dark:prose-invert">
        <MarkdownRenderer htmlContent={content} />
      </div>
    </div>
  );
}

export async function generateStaticParams() {
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

  const params = files.map((file) => ({
    slug: file.split('/'),
  }));

  // Add empty slug for /docs route
  params.push({ slug: [] });

  return params;
}
