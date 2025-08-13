import React, { useState, memo, useMemo } from 'react';
import ReactMarkdown from 'react-markdown';
import { Prism as SyntaxHighlighter } from 'react-syntax-highlighter';
import { dark } from 'react-syntax-highlighter/dist/esm/styles/prism';
import remarkGfm from 'remark-gfm';
import rehypeHighlight from 'rehype-highlight';
import { Copy, Check } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { cn } from '@/lib/utils';
import toast from 'react-hot-toast';

interface MarkdownRendererProps {
  content: string;
  className?: string;
}

interface CodeBlockProps {
  language?: string;
  value: string;
}

const CodeBlock: React.FC<CodeBlockProps> = ({ language, value }) => {
  const [copied, setCopied] = useState(false);

  const handleCopy = async () => {
    try {
      await navigator.clipboard.writeText(value);
      setCopied(true);
      toast.success('Code copied to clipboard!');
      setTimeout(() => setCopied(false), 2000);
    } catch (error) {
      toast.error('Failed to copy code');
    }
  };

  return (
    <div className="relative group">
      <Button
        size="sm"
        variant="ghost"
        className="absolute top-2 right-2 h-8 w-8 p-0 opacity-0 group-hover:opacity-100 transition-opacity"
        onClick={handleCopy}
      >
        {copied ? (
          <Check className="h-4 w-4 text-green-500" />
        ) : (
          <Copy className="h-4 w-4" />
        )}
      </Button>
      <SyntaxHighlighter
        style={dark}
        language={language || 'text'}
        PreTag="div"
        className="rounded-md"
      >
        {String(value).replace(/\n$/, '')}
      </SyntaxHighlighter>
    </div>
  );
};

export const MarkdownRenderer: React.FC<MarkdownRendererProps> = memo(({
  content,
  className,
}) => {
  const components = useMemo(() => ({
    code({ node, className, children, ...props }) {
      const match = /language-(\w+)/.exec(className || '');
      const inline = !match;
      const language = match ? match[1] : undefined;
      
      // Extract text content from AST node
      const extractTextFromNode = (node: any): string => {
        if (!node) return '';
        if (node.type === 'text') return node.value || '';
        if (node.children) {
          return node.children.map(extractTextFromNode).join('');
        }
        return '';
      };
      
      const value = extractTextFromNode(node).replace(/\n$/, '');

      return !inline ? (
        <CodeBlock language={language} value={value} />
      ) : (
        <code
          className="relative rounded bg-muted px-[0.3rem] py-[0.2rem] font-mono text-sm font-semibold"
          {...props}
        >
          {children}
        </code>
      );
    },
    pre({ children }) {
      return <>{children}</>;
    },
    table({ children }) {
      return (
        <div className="my-6 w-full overflow-y-auto">
          <table className="w-full border-collapse border border-border">
            {children}
          </table>
        </div>
      );
    },
    th({ children }) {
      return (
        <th className="border border-border px-4 py-2 text-left font-bold bg-muted">
          {children}
        </th>
      );
    },
    td({ children }) {
      return (
        <td className="border border-border px-4 py-2">
          {children}
        </td>
      );
    },
    blockquote({ children }) {
      return (
        <blockquote className="mt-6 border-l-2 border-border pl-6 italic">
          {children}
        </blockquote>
      );
    },
    a({ href, children }) {
      return (
        <a
          href={href}
          className="font-medium text-primary underline underline-offset-4 hover:no-underline"
          target="_blank"
          rel="noopener noreferrer"
        >
          {children}
        </a>
      );
    },
  }), []);

  // Memoize the markdown processing to avoid re-parsing identical content
  const memoizedMarkdown = useMemo(() => (
    <ReactMarkdown
      remarkPlugins={[remarkGfm]}
      rehypePlugins={[rehypeHighlight]}
      components={components}
    >
      {content}
    </ReactMarkdown>
  ), [content, components]);

  return (
    <div className={cn("prose prose-sm dark:prose-invert max-w-none", className)}>
      {memoizedMarkdown}
    </div>
  );
}, (prevProps, nextProps) => {
  // Custom comparison function to optimize re-renders
  return prevProps.content === nextProps.content && prevProps.className === nextProps.className;
});