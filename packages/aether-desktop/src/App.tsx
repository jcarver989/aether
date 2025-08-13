import React, { useEffect, useState } from 'react';
import { AppContextProvider } from './components/AppContextProvider';
import { ChatView } from './components/ChatView';
import { ChatInput } from './components/ChatInput';
import { ProviderSettings } from './components/ProviderSettings';
import { useAppContext } from './hooks/useAppContext';
import { Button } from './components/ui/button';
import { Settings } from 'lucide-react';
import { Toaster } from 'react-hot-toast';
import "./App.css";

function AppContent() {
  const { actions } = useAppContext();
  const [settingsOpen, setSettingsOpen] = useState(false);

  useEffect(() => {
    actions.init();
    // Ensure dark mode is applied to html element
    document.documentElement.classList.add('dark');
  }, [actions]);

  return (
    <div className="flex flex-col h-screen bg-background text-foreground">
      <header className="sticky top-0 z-50 border-b border-border/40 bg-background/95 backdrop-blur supports-[backdrop-filter]:bg-background/60">
        <div className="flex h-16 items-center justify-between px-6">
          <div className="flex items-center space-x-2">
            <div className="h-8 w-8 rounded-lg bg-gradient-to-br from-primary to-primary/80 flex items-center justify-center">
              <span className="text-primary-foreground font-bold text-sm">A</span>
            </div>
            <h1 className="text-xl font-semibold tracking-tight">Aether</h1>
          </div>
          <Button
            variant="ghost"
            size="sm"
            onClick={() => setSettingsOpen(true)}
            className="h-9 w-9 p-0 hover:bg-accent transition-colors"
          >
            <Settings className="h-4 w-4" />
          </Button>
        </div>
      </header>
      
      <div className="flex flex-1 overflow-hidden">
        <main className="flex flex-1 flex-col">
          <ChatView className="flex-1" />
          <ChatInput />
        </main>
      </div>

      <ProviderSettings 
        open={settingsOpen} 
        onOpenChange={setSettingsOpen} 
      />
    </div>
  );
}

function App() {
  return (
    <AppContextProvider>
      <AppContent />
      
      <Toaster
        position="top-right"
        toastOptions={{
          duration: 3000,
          style: {
            background: 'hsl(var(--background))',
            color: 'hsl(var(--foreground))',
            border: '1px solid hsl(var(--border))',
          },
          success: {
            iconTheme: {
              primary: 'hsl(var(--primary))',
              secondary: 'hsl(var(--primary-foreground))',
            },
          },
          error: {
            iconTheme: {
              primary: 'hsl(var(--destructive))',
              secondary: 'hsl(var(--destructive-foreground))',
            },
          },
        }}
      />
    </AppContextProvider>
  );
}

export default App;
