// Custom syntax highlighting theme to match Aether's futuristic sci-fi aesthetic
import { PrismTheme } from 'prism-react-renderer';

export const aetherSyntaxTheme: PrismTheme = {
  plain: {
    color: 'hsl(200, 20%, 98%)', // --foreground
    backgroundColor: 'hsl(210, 20%, 16%)', // --muted
  },
  styles: [
    {
      types: ['comment', 'prolog', 'doctype', 'cdata'],
      style: {
        color: 'hsl(200, 15%, 55%)', // Dimmed foreground for comments
        fontStyle: 'italic',
      },
    },
    {
      types: ['punctuation'],
      style: {
        color: 'hsl(200, 20%, 85%)', // Slightly dimmed foreground
      },
    },
    {
      types: ['property', 'tag', 'boolean', 'number', 'constant', 'symbol', 'deleted'],
      style: {
        color: 'hsl(180, 100%, 75%)', // --primary (cyan/aqua)
      },
    },
    {
      types: ['selector', 'attr-name', 'string', 'char', 'builtin', 'inserted'],
      style: {
        color: 'hsl(120, 85%, 70%)', // Green for strings
      },
    },
    {
      types: ['operator', 'entity', 'url'],
      style: {
        color: 'hsl(285, 100%, 80%)', // --accent (purple/magenta)
      },
    },
    {
      types: ['atrule', 'attr-value', 'keyword'],
      style: {
        color: 'hsl(315, 85%, 75%)', // Pink/magenta for keywords
      },
    },
    {
      types: ['function', 'class-name'],
      style: {
        color: 'hsl(45, 100%, 75%)', // Yellow/gold for functions and classes
      },
    },
    {
      types: ['regex', 'important', 'variable'],
      style: {
        color: 'hsl(25, 100%, 70%)', // Orange for regex and variables
      },
    },
    {
      types: ['important', 'bold'],
      style: {
        fontWeight: 'bold',
      },
    },
    {
      types: ['italic'],
      style: {
        fontStyle: 'italic',
      },
    },
    {
      types: ['entity'],
      style: {
        cursor: 'help',
      },
    },
    {
      types: ['namespace'],
      style: {
        opacity: 0.7,
      },
    },
  ],
};

// Alternative theme using react-syntax-highlighter format
export const aetherPrismTheme = {
  'code[class*="language-"]': {
    color: 'hsl(200, 20%, 98%)',
    background: 'hsl(210, 20%, 16%)',
    textShadow: 'none',
    fontFamily: '"JetBrains Mono", "Fira Code", "Cascadia Code", "SF Mono", "Monaco", "Roboto Mono", monospace',
    fontSize: '14px',
    lineHeight: '1.5',
    direction: 'ltr' as 'ltr',
    textAlign: 'left',
    whiteSpace: 'pre',
    wordSpacing: 'normal',
    wordBreak: 'normal',
    wordWrap: 'normal',
    MozTabSize: '2',
    OTabSize: '2',
    tabSize: '2',
    WebkitHyphens: 'none',
    MozHyphens: 'none',
    msHyphens: 'none',
    hyphens: 'none' as 'none',
  },
  'pre[class*="language-"]': {
    color: 'hsl(200, 20%, 98%)',
    background: 'linear-gradient(135deg, hsl(210, 20%, 16%), hsl(210, 20%, 14%))',
    textShadow: 'none',
    fontFamily: '"JetBrains Mono", "Fira Code", "Cascadia Code", "SF Mono", "Monaco", "Roboto Mono", monospace',
    fontSize: '14px',
    lineHeight: '1.5',
    direction: 'ltr' as 'ltr',
    textAlign: 'left',
    whiteSpace: 'pre',
    wordSpacing: 'normal',
    wordBreak: 'normal',
    wordWrap: 'normal',
    MozTabSize: '2',
    OTabSize: '2',
    tabSize: '2',
    WebkitHyphens: 'none',
    MozHyphens: 'none',
    msHyphens: 'none',
    hyphens: 'none' as 'none',
    padding: '1em',
    margin: '0.5em 0',
    overflow: 'auto',
    border: '1px solid hsl(195, 40%, 30%)',
    borderLeft: '3px solid hsl(180, 100%, 75%)',
    borderRadius: '0.125rem',
    boxShadow: '0 0 20px hsl(180, 100%, 75%, 0.1)',
  },
  'pre[class*="language-"] *': {
    border: 'none !important',
    outline: 'none !important',
  },
  ':not(pre) > code[class*="language-"]': {
    background: 'hsl(210, 20%, 16%)',
    padding: '0.2em 0.4em',
    borderRadius: '0.125rem',
    border: '1px solid hsl(195, 40%, 30%, 0.5)',
  },
  'comment': {
    color: 'hsl(200, 15%, 55%)',
    fontStyle: 'italic',
  },
  'prolog': {
    color: 'hsl(200, 15%, 55%)',
  },
  'doctype': {
    color: 'hsl(200, 15%, 55%)',
  },
  'cdata': {
    color: 'hsl(200, 15%, 55%)',
  },
  'punctuation': {
    color: 'hsl(200, 20%, 85%)',
  },
  'property': {
    color: 'hsl(180, 100%, 75%)', // Primary cyan
  },
  'tag': {
    color: 'hsl(180, 100%, 75%)', // Primary cyan
  },
  'boolean': {
    color: 'hsl(180, 100%, 75%)', // Primary cyan
  },
  'number': {
    color: 'hsl(180, 100%, 75%)', // Primary cyan
  },
  'constant': {
    color: 'hsl(180, 100%, 75%)', // Primary cyan
  },
  'symbol': {
    color: 'hsl(180, 100%, 75%)', // Primary cyan
  },
  'deleted': {
    color: 'hsl(0, 90%, 70%)', // Destructive red
  },
  'selector': {
    color: 'hsl(120, 85%, 70%)', // Green
  },
  'attr-name': {
    color: 'hsl(120, 85%, 70%)', // Green
  },
  'string': {
    color: 'hsl(120, 85%, 70%)', // Green
  },
  'char': {
    color: 'hsl(120, 85%, 70%)', // Green
  },
  'builtin': {
    color: 'hsl(120, 85%, 70%)', // Green
  },
  'inserted': {
    color: 'hsl(120, 85%, 70%)', // Green
  },
  'operator': {
    color: 'hsl(285, 100%, 80%)', // Accent purple
  },
  'entity': {
    color: 'hsl(285, 100%, 80%)', // Accent purple
    cursor: 'help',
  },
  'url': {
    color: 'hsl(285, 100%, 80%)', // Accent purple
  },
  'atrule': {
    color: 'hsl(315, 85%, 75%)', // Pink for keywords
  },
  'attr-value': {
    color: 'hsl(315, 85%, 75%)', // Pink for keywords
  },
  'keyword': {
    color: 'hsl(315, 85%, 75%)', // Pink for keywords
  },
  'function': {
    color: 'hsl(45, 100%, 75%)', // Yellow/gold
  },
  'class-name': {
    color: 'hsl(45, 100%, 75%)', // Yellow/gold
  },
  'regex': {
    color: 'hsl(25, 100%, 70%)', // Orange
  },
  'important': {
    color: 'hsl(25, 100%, 70%)', // Orange
    fontWeight: 'bold',
  },
  'variable': {
    color: 'hsl(25, 100%, 70%)', // Orange
  },
  'bold': {
    fontWeight: 'bold',
  },
  'italic': {
    fontStyle: 'italic',
  },
  'namespace': {
    opacity: 0.7,
  },
};