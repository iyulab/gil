import fs from 'fs';
import path from 'path';
import { cache } from 'react';
// import { marked } from 'marked';
import OptimizedSidebar from '../../components/OptimizedSidebar';
import MarkdownRenderer from '../../components/MarkdownRenderer'; // New client component

interface DocEntry {
  name: string;
  slug: string[];
  path: string;
  isFolder: boolean;
  children?: DocEntry[];
}

// Document priority configuration
const docPriority = {
  high: [
    { file: 'OVERVIEW.md', label: 'Overview', icon: '📋' },
    { file: 'YAML_SPEC.md', label: 'YAML Specification', icon: '📝' },
    { file: 'NODE_SPEC.md', label: 'Node Specification', icon: '🔧' },
  ],
  medium: [
    { file: 'ARCHITECTURE.md', label: 'Architecture', icon: '🏗️' },
    { file: 'DEV.md', label: 'Development Guide', icon: '💻' },
    { file: 'CONTEXT_SYSTEM.md', label: 'Context System', icon: '🔗' },
  ],
  nodes: [
    { file: 'nodes', label: 'Nodes', icon: '🧩' },
  ],
  low: [
    { file: 'TASKS.md', label: 'Tasks', icon: '✅' },
  ]
};

// Flattened priority order for sorting
const docOrder = [
  ...docPriority.high,
  ...docPriority.medium,
  ...docPriority.nodes,
  ...docPriority.low,
];

const getDocTree = cache((dirPath: string, baseSlug: string[] = []): DocEntry[] => {
  const entries: DocEntry[] = [];
  const files = fs.readdirSync(dirPath, { withFileTypes: true });

  // Custom sort for nodes directory to put README.md first
  if (baseSlug.includes('nodes')) {
    files.sort((a, b) => {
      if (a.name === 'README.md') return -1;
      if (b.name === 'README.md') return 1;
      return a.name.localeCompare(b.name);
    });
  }

  const orderedFiles = docOrder
    .map(item => files.find(file => file.name === item.file))
    .filter((file): file is fs.Dirent => !!file);

  const remainingFiles = files.filter(file => !docOrder.some(item => item.file === file.name));

  const allFiles = [...orderedFiles, ...remainingFiles];

  allFiles.forEach(file => {
    const fullPath = path.join(dirPath, file.name);
    const slug = [...baseSlug, file.name.replace(/\.md$/, '')];
    const docConfig = docOrder.find(item => item.file === file.name);
    let name = docConfig ? docConfig.label : file.name.replace(/\.md$/, '');

    if (file.name === 'README.md' && baseSlug.includes('nodes')) {
      name = 'Overview'; // Special label for nodes/README.md
    }

    if (file.isDirectory()) {
      entries.push({
        name,
        slug: slug,
        path: fullPath,
        isFolder: true,
        children: getDocTree(fullPath, slug),
      });
    } else if (file.isFile() && file.name.endsWith('.md')) {
      entries.push({
        name,
        slug: slug,
        path: fullPath,
        isFolder: false,
      });
    }
  });

  return entries;
});

const docsPath = path.join(process.cwd(), '..', 'docs');
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
