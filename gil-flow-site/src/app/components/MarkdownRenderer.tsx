"use client";

import React, { useEffect, useState, useRef } from 'react';
import './markdown.css';
import mermaid from 'mermaid';
import { marked } from 'marked';
import DOMPurify from 'dompurify';

interface MarkdownRendererProps {
  htmlContent: string; // This is raw markdown content
}

export default function MarkdownRenderer({ htmlContent }: MarkdownRendererProps) {
  const [renderedHtml, setRenderedHtml] = useState('');
  const [mermaidPlaceholders, setMermaidPlaceholders] = useState<{ [key: string]: string }>({});
  const contentRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    console.log('MarkdownRenderer: htmlContent received', htmlContent?.substring(0, 200) + '...');

    const processMarkdown = async () => {
      const mermaidRegex = /```mermaid\n([\s\S]*?)\n```/g;
      let processedMarkdown = htmlContent;
      const currentMermaidPlaceholders: { [key: string]: string } = {};
      let index = 0;

      // Extract mermaid code and replace with unique div placeholders
      processedMarkdown = processedMarkdown.replace(mermaidRegex, (match, code) => {
        const id = `mermaid-diagram-${index}`;
        currentMermaidPlaceholders[id] = code;
        index++;
        console.log(`MarkdownRenderer: Extracted Mermaid diagram ${id}:`, code.substring(0, 100) + '...');
        return `<div id="${id}" class="mermaid-placeholder"></div>`;
      });

      console.log('MarkdownRenderer: processedMarkdown after regex', processedMarkdown.substring(0, 200) + '...');

      // Convert markdown (with div placeholders) to HTML
      const html = await marked.parse(processedMarkdown);
      console.log('MarkdownRenderer: HTML after marked.parse', html.substring(0, 200) + '...');

      const cleanHtml = DOMPurify.sanitize(html);
      console.log('MarkdownRenderer: cleanHtml after DOMPurify.sanitize', cleanHtml.substring(0, 200) + '...');
      setRenderedHtml(cleanHtml);
      setMermaidPlaceholders(currentMermaidPlaceholders);

      console.log('MarkdownRenderer: mermaidPlaceholders state set', currentMermaidPlaceholders);
    };

    processMarkdown();
  }, [htmlContent]);

  useEffect(() => {
    if (contentRef.current && renderedHtml && Object.keys(mermaidPlaceholders).length > 0) {
      console.log('MarkdownRenderer: contentRef.current and renderedHtml are available for Mermaid rendering.');
      
      // Initialize Mermaid with better configuration
      mermaid.initialize({
        startOnLoad: false,
        theme: 'default',
        securityLevel: 'loose',
        fontFamily: 'monospace',
        fontSize: 16,
        sequence: {
          actorMargin: 50,
          width: 150,
          height: 65,
          boxMargin: 10,
          boxTextMargin: 5,
          noteMargin: 10,
          messageMargin: 35
        },
        flowchart: {
          htmlLabels: true,
          curve: 'basis',
          padding: 20
        }
      });

      // Process each Mermaid diagram
      Object.entries(mermaidPlaceholders).forEach(([id, diagramCode]) => {
        const placeholderElement = contentRef.current!.querySelector(`#${id}`);
        console.log(`MarkdownRenderer: Looking for placeholder #${id}. Found:`, !!placeholderElement);

        if (placeholderElement) {
          console.log(`MarkdownRenderer: Attempting to render Mermaid for ${id}. Code:`, diagramCode.substring(0, 100) + '...');
          
          // Clean up the diagram code
          const cleanDiagramCode = diagramCode.trim();
          
          try {
            mermaid.render(`${id}-svg`, cleanDiagramCode)
              .then(({ svg }) => {
                console.log(`MarkdownRenderer: Successfully rendered Mermaid for ${id}`);
                placeholderElement.innerHTML = `<div class="mermaid-diagram">${svg}</div>`;
                placeholderElement.classList.remove('mermaid-placeholder');
              })
              .catch(error => {
                console.error('Mermaid rendering error for', id, ':', error);
                placeholderElement.innerHTML = `
                  <div class="mermaid-error">
                    <h4>⚠️ Mermaid Diagram Error</h4>
                    <p>Unable to render the diagram. Here's the original code:</p>
                    <pre><code>${cleanDiagramCode}</code></pre>
                    <details>
                      <summary>Error details</summary>
                      <pre><code>${error.message || error}</code></pre>
                    </details>
                  </div>
                `;
                placeholderElement.classList.remove('mermaid-placeholder');
              });
          } catch (error) {
            console.error('Mermaid render call error for', id, ':', error);
            placeholderElement.innerHTML = `
              <div class="mermaid-error">
                <h4>⚠️ Mermaid Diagram Error</h4>
                <p>Unable to render the diagram. Here's the original code:</p>
                <pre><code>${cleanDiagramCode}</code></pre>
                <details>
                  <summary>Error details</summary>
                  <pre><code>${error instanceof Error ? error.message : String(error)}</code></pre>
                </details>
              </div>
            `;
            placeholderElement.classList.remove('mermaid-placeholder');
          }
        }
      });
    } else if (Object.keys(mermaidPlaceholders).length === 0) {
      console.log('MarkdownRenderer: No Mermaid diagrams to render.');
    } else {
      console.log('MarkdownRenderer: contentRef.current or renderedHtml NOT available for Mermaid rendering yet.');
    }
  }, [renderedHtml, mermaidPlaceholders]); // This effect runs after renderedHtml and mermaidPlaceholders are updated

  return <div ref={contentRef} className="prose dark:prose-invert" dangerouslySetInnerHTML={{ __html: renderedHtml }} />;
}
