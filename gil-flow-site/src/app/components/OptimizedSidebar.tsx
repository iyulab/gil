"use client";

import Link from 'next/link';
import React, { useState, useEffect, useMemo, useCallback } from 'react';
// Document priority configuration - organized by importance and user journey
const docPriority = {
  // High priority - essential docs for getting started
  high: [
    { file: 'OVERVIEW.md', label: 'Overview', icon: '📋' },
    { file: 'YAML_SPEC.md', label: 'YAML Specification', icon: '📝' },
    { file: 'NODE_SPEC.md', label: 'Node Specification', icon: '🔧' },
  ],
  // Medium priority - important for development
  medium: [
    { file: 'ARCHITECTURE.md', label: 'Architecture', icon: '🏗️' },
    { file: 'DEV.md', label: 'Development Guide', icon: '💻' },
    { file: 'CONTEXT_SYSTEM.md', label: 'Context System', icon: '🔗' },
  ],
  // Nodes - special category that's always expandable
  nodes: [
    { file: 'nodes', label: 'Nodes', icon: '🧩' },
  ],
  // Low priority - additional resources
  low: [
    { file: 'TASKS.md', label: 'Tasks', icon: '✅' },
  ]
};

interface DocEntry {
  name: string;
  slug: string[];
  path: string;
  isFolder: boolean;
  children?: DocEntry[];
}

interface SidebarProps {
  currentSlug?: string[];
  docTree: DocEntry[];
}

interface PrioritySection {
  title: string;
  icon: string;
  items: DocEntry[];
  defaultExpanded?: boolean;
}

// Stable component for rendering individual document links
const DocLink = React.memo(({ 
  entry, 
  currentSlug, 
  className = "" 
}: { 
  entry: DocEntry; 
  currentSlug: string[]; 
  className?: string; 
}) => {
  const isActive = currentSlug.join('/') === entry.slug.join('/');
  
  return (
    <Link 
      href={`/docs/${entry.slug.join('/')}`}
      className={`block px-3 py-2 rounded-md text-sm transition-colors ${
        isActive
          ? 'bg-blue-100 text-blue-700 dark:bg-blue-900 dark:text-blue-200 font-medium'
          : 'text-gray-600 hover:text-gray-900 hover:bg-gray-50 dark:text-gray-300 dark:hover:text-white dark:hover:bg-gray-700'
      } ${className}`}
    >
      {entry.name}
    </Link>
  );
});

DocLink.displayName = 'DocLink';

// Stable component for rendering folder sections
const FolderSection = React.memo(({ 
  entry, 
  currentSlug, 
  isExpanded, 
  onToggle, 
  icon 
}: { 
  entry: DocEntry; 
  currentSlug: string[]; 
  isExpanded: boolean; 
  onToggle: (path: string) => void; 
  icon?: string;
}) => {
  const handleToggle = useCallback(() => {
    onToggle(entry.path);
  }, [entry.path, onToggle]);

  return (
    <div className="mb-2">
      <button
        onClick={handleToggle}
        className="flex items-center w-full px-3 py-2 text-left text-sm font-medium text-gray-700 hover:text-gray-900 hover:bg-gray-50 dark:text-gray-300 dark:hover:text-white dark:hover:bg-gray-700 rounded-md transition-colors"
      >
        <span className="mr-2 text-base">{icon || '📁'}</span>
        <span className="flex-1">{entry.name}</span>
        <span className={`ml-2 transition-transform ${isExpanded ? 'rotate-90' : ''}`}>
          ▶
        </span>
      </button>
      {isExpanded && entry.children && (
        <div className="ml-6 mt-2 border-l border-gray-200 dark:border-gray-600">
          {entry.children.map(child => (
            <div key={child.path} className="ml-4">
              <DocLink 
                entry={child} 
                currentSlug={currentSlug}
                className="pl-2"
              />
            </div>
          ))}
        </div>
      )}
    </div>
  );
});

FolderSection.displayName = 'FolderSection';

// Stable component for rendering priority sections
const PrioritySection = React.memo(({ 
  section, 
  currentSlug, 
  expandedFolders, 
  onToggleFolder 
}: { 
  section: PrioritySection; 
  currentSlug: string[]; 
  expandedFolders: Set<string>; 
  onToggleFolder: (path: string) => void; 
}) => {
  const [isExpanded, setIsExpanded] = useState(section.defaultExpanded ?? true);

  const toggleSection = useCallback(() => {
    setIsExpanded(prev => !prev);
  }, []);

  return (
    <div className="mb-6">
      <button
        onClick={toggleSection}
        className="flex items-center w-full px-2 py-2 text-left text-xs font-semibold text-gray-500 hover:text-gray-700 dark:text-gray-400 dark:hover:text-gray-200 uppercase tracking-wider"
      >
        <span className="mr-2">{section.icon}</span>
        <span className="flex-1">{section.title}</span>
        <span className={`ml-2 transition-transform ${isExpanded ? 'rotate-90' : ''}`}>
          ▶
        </span>
      </button>
      {isExpanded && (
        <div className="mt-2 space-y-1">
          {section.items.map(item => (
            item.isFolder ? (
              <FolderSection
                key={item.path}
                entry={item}
                currentSlug={currentSlug}
                isExpanded={expandedFolders.has(item.path)}
                onToggle={onToggleFolder}
                icon={docPriority.nodes.find(n => n.file === 'nodes')?.icon}
              />
            ) : (
              <DocLink
                key={item.path}
                entry={item}
                currentSlug={currentSlug}
              />
            )
          ))}
        </div>
      )}
    </div>
  );
});

PrioritySection.displayName = 'PrioritySection';

export default function OptimizedSidebar({ currentSlug = [], docTree }: SidebarProps) {
  const [expandedFolders, setExpandedFolders] = useState<Set<string>>(new Set());

  // Load expanded state from localStorage on mount
  useEffect(() => {
    const stored = localStorage.getItem('gill-docs-expanded-folders');
    if (stored) {
      try {
        const parsed = JSON.parse(stored);
        setExpandedFolders(new Set(parsed));
      } catch {
        // Ignore parsing errors
      }
    }
  }, []);

  // Stable toggle function
  const toggleFolder = useCallback((folderPath: string) => {
    setExpandedFolders(prev => {
      const newSet = new Set(prev);
      if (newSet.has(folderPath)) {
        newSet.delete(folderPath);
      } else {
        newSet.add(folderPath);
      }
      
      // Persist to localStorage
      localStorage.setItem('gill-docs-expanded-folders', JSON.stringify(Array.from(newSet)));
      return newSet;
    });
  }, []);

  // Organize documents by priority using stable memoization
  const prioritySections = useMemo(() => {
    // Create a map for fast lookup
    const docMap = new Map<string, DocEntry>();
    const buildDocMap = (entries: DocEntry[]) => {
      entries.forEach(entry => {
        const fileName = entry.slug[entry.slug.length - 1] + '.md';
        docMap.set(fileName, entry);
        docMap.set(entry.slug[entry.slug.length - 1], entry); // Also map without .md
        if (entry.children) {
          buildDocMap(entry.children);
        }
      });
    };
    buildDocMap(docTree);

    const sections: PrioritySection[] = [
      {
        title: 'Getting Started',
        icon: '🚀',
        items: docPriority.high.map(doc => docMap.get(doc.file) || docMap.get(doc.file.replace('.md', ''))).filter(Boolean) as DocEntry[],
        defaultExpanded: true
      },
      {
        title: 'Development',
        icon: '⚙️',
        items: docPriority.medium.map(doc => docMap.get(doc.file) || docMap.get(doc.file.replace('.md', ''))).filter(Boolean) as DocEntry[],
        defaultExpanded: true
      },
      {
        title: 'Node Reference',
        icon: '🧩',
        items: docPriority.nodes.map(doc => docMap.get(doc.file) || docMap.get(doc.file.replace('.md', ''))).filter(Boolean) as DocEntry[],
        defaultExpanded: true
      }
    ];

    // Add any remaining documents that weren't categorized
    const categorizedFiles = new Set([
      ...docPriority.high.map(d => d.file),
      ...docPriority.medium.map(d => d.file),
      ...docPriority.nodes.map(d => d.file),
      ...docPriority.low.map(d => d.file)
    ]);

    const uncategorizedDocs = docTree.filter(doc => {
      const fileName = doc.slug[doc.slug.length - 1] + '.md';
      return !categorizedFiles.has(fileName) && !categorizedFiles.has(doc.slug[doc.slug.length - 1]);
    });

    if (uncategorizedDocs.length > 0) {
      sections.push({
        title: 'Other',
        icon: '📄',
        items: uncategorizedDocs,
        defaultExpanded: false
      });
    }

    return sections;
  }, [docTree]);

  return (
    <aside className="w-64 bg-white dark:bg-gray-800 border-r border-gray-200 dark:border-gray-700 h-full flex flex-col">
      <div className="p-4 border-b border-gray-200 dark:border-gray-700">
        <h2 className="text-lg font-semibold text-gray-900 dark:text-white">
          Gil Documentation
        </h2>
      </div>
      
      <nav className="flex-1 overflow-y-auto p-4">
        {prioritySections.map((section, index) => (
          <PrioritySection
            key={`${section.title}-${index}`}
            section={section}
            currentSlug={currentSlug}
            expandedFolders={expandedFolders}
            onToggleFolder={toggleFolder}
          />
        ))}
      </nav>
      
      <div className="p-4 border-t border-gray-200 dark:border-gray-700 text-xs text-gray-500 dark:text-gray-400">
        Gil-Flow Documentation
      </div>
    </aside>
  );
}