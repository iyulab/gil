"use client";

import Link from 'next/link';
import React, { useState, useEffect, useCallback, memo } from 'react';

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

const MemoizedRenderTree = memo(function RenderTree(
  {
    tree,
    currentSlug,
    expandedFolders,
    toggleFolder
  }: {
    tree: DocEntry[];
    currentSlug: string[];
    expandedFolders: Set<string>;
    toggleFolder: (path: string) => void;
  }
) {
  return (
    <ul>
      {tree.map(entry => (
        <li key={entry.path} className="mb-2">
          {entry.isFolder ? (
            <details
              className="text-gray-700 dark:text-gray-300"
              open={expandedFolders.has(entry.path)}
              onToggle={(e) => {
                e.preventDefault();
                toggleFolder(entry.path);
              }}
            >
              <summary className="cursor-pointer hover:text-blue-600 dark:hover:text-blue-400 font-medium">
                {entry.name}
              </summary>
              <div className="ml-4 border-l border-gray-200 dark:border-gray-700 pl-4">
                {entry.children && (
                  <MemoizedRenderTree
                    tree={entry.children}
                    currentSlug={currentSlug}
                    expandedFolders={expandedFolders}
                    toggleFolder={toggleFolder}
                  />
                )}
              </div>
            </details>
          ) : (
            <Link href={`/docs/${entry.slug.join('/')}`}>
              <p className={`block hover:text-blue-600 dark:hover:text-blue-400 ${
                currentSlug.join('/') === entry.slug.join('/')
                  ? 'font-bold text-blue-600 dark:text-blue-400'
                  : 'text-gray-600 dark:text-gray-300'
              }`}>
                {entry.name}
              </p>
            </Link>
          )}
        </li>
      ))}
    </ul>
  );
});

export default function Sidebar({ currentSlug = [], docTree }: SidebarProps) {
  const [expandedFolders, setExpandedFolders] = useState<Set<string>>(new Set());

  // Load expanded state from localStorage on initial mount
  useEffect(() => {
    const stored = localStorage.getItem('expandedFolders');
    if (stored) {
      try {
        const parsed = JSON.parse(stored);
        setExpandedFolders(new Set(parsed));
      } catch {
        setExpandedFolders(new Set());
      }
    }
  }, []);

  const toggleFolder = useCallback((folderPath: string) => {
    setExpandedFolders(prev => {
      const newSet = new Set(prev);
      if (newSet.has(folderPath)) {
        newSet.delete(folderPath);
      } else {
        newSet.add(folderPath);
      }

      // Check if the content of the set has actually changed
      if (newSet.size === prev.size && Array.from(newSet).every(item => prev.has(item))) {
        return prev; // No actual change, return previous state to prevent re-render
      }

      // Persist to localStorage
      localStorage.setItem('expandedFolders', JSON.stringify(Array.from(newSet)));
      return newSet;
    });
  }, []);

  return (
    <aside className="w-64 bg-gray-50 dark:bg-gray-800 p-4 overflow-y-auto h-full border-r border-gray-200 dark:border-gray-700">
      <h2 className="text-xl font-bold mb-4 text-gray-900 dark:text-white">Documentation</h2>
      <nav>
        <MemoizedRenderTree
          tree={docTree}
          currentSlug={currentSlug}
          expandedFolders={expandedFolders}
          toggleFolder={toggleFolder}
        />
      </nav>
    </aside>
  );
}
